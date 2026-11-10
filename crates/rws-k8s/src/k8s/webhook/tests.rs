use k8s_openapi::api::core::v1::ConfigMap;
use kube::ResourceExt;
use serde_json::{json, Value};

use super::*;
use rws_server::test_client::TestClient;

/// An AdmissionReview as the API server sends it.
fn review(operation: &str, object: Value) -> String {
    json!({
        "apiVersion": "admission.k8s.io/v1",
        "kind": "AdmissionReview",
        "request": {
            "uid": "705ab4f5-6393-11e8-b7cc-42010a800002",
            "kind": {"group": "", "version": "v1", "kind": "ConfigMap"},
            "resource": {"group": "", "version": "v1", "resource": "configmaps"},
            "requestKind": {"group": "", "version": "v1", "kind": "ConfigMap"},
            "requestResource": {"group": "", "version": "v1", "resource": "configmaps"},
            "name": "settings",
            "namespace": "default",
            "operation": operation,
            "userInfo": {"username": "ann", "groups": ["system:authenticated"]},
            "object": object,
            "oldObject": null,
            "dryRun": false,
            "options": null
        }
    })
    .to_string()
}

fn cm(data: Value) -> Value {
    json!({"apiVersion": "v1", "kind": "ConfigMap", "metadata": {"name": "settings", "namespace": "default"}, "data": data})
}

fn webhooks() -> Webhooks {
    Webhooks::new()
        .validate::<ConfigMap, _>("/validate", |r| {
            let has_owner = r.object.as_ref().and_then(|o| o.data.as_ref()).is_some_and(|d| d.contains_key("owner"));
            if has_owner { Ok(()) } else { Err(format!("{} must say its owner", r.name)) }
        })
        .mutate::<ConfigMap, _>("/mutate", |mut c| {
            c.labels_mut().insert("managed".into(), "yes".into());
            Ok(c)
        })
        .convert("/convert", |mut o, desired| {
            if desired == "example.com/v2" {
                o["spec"]["replicas"] = o["spec"]["size"].take();
                Ok(o)
            } else {
                Err(format!("can't convert to {desired}"))
            }
        })
}

fn post(client: &TestClient<Webhooks>, path: &str, body: &str) -> (i16, Value) {
    let r = client.post(path).body_json(body).send();
    (r.status(), serde_json::from_slice(r.body_bytes()).unwrap_or(Value::Null))
}

#[test]
fn validating_allows_and_denies() {
    let client = TestClient::new(webhooks());
    let (status, ok) = post(&client, "/validate", &review("CREATE", cm(json!({"owner": "team-a"}))));
    assert_eq!(200, status);
    assert_eq!(("705ab4f5-6393-11e8-b7cc-42010a800002", true), (ok["response"]["uid"].as_str().unwrap(), ok["response"]["allowed"].as_bool().unwrap()));
    assert_eq!("AdmissionReview", ok["kind"]);
    let (_, denied) = post(&client, "/validate", &review("CREATE", cm(json!({}))));
    assert_eq!(false, denied["response"]["allowed"]);
    assert_eq!("settings must say its owner", denied["response"]["status"]["message"]);
}

#[test]
fn mutating_answers_a_json_patch() {
    let client = TestClient::new(webhooks());
    let (_, answer) = post(&client, "/mutate", &review("CREATE", cm(json!({"a": "1"}))));
    assert_eq!(true, answer["response"]["allowed"]);
    assert_eq!("JSONPatch", answer["response"]["patchType"]);
    let patch_b64 = answer["response"]["patch"].as_str().unwrap();
    let patch = String::from_utf8(base64_decode(patch_b64)).unwrap();
    let ops: Value = serde_json::from_str(&patch).unwrap();
    assert_eq!(json!([{"op": "add", "path": "/metadata/labels", "value": {"managed": "yes"}}]), ops);
    // No change on delete: no patch.
    let (_, delete) = post(&client, "/mutate", &review("DELETE", cm(json!({}))));
    assert!(delete["response"]["patch"].is_null());
}

#[test]
fn conversion_converts_each_object() {
    let client = TestClient::new(webhooks());
    let body = json!({
        "apiVersion": "apiextensions.k8s.io/v1",
        "kind": "ConversionReview",
        "request": {"uid": "u1", "desiredAPIVersion": "example.com/v2", "objects": [
            {"apiVersion": "example.com/v1", "kind": "Widget", "metadata": {"name": "a"}, "spec": {"size": 3}}
        ]}
    });
    let (status, answer) = post(&client, "/convert", &body.to_string());
    assert_eq!(200, status);
    assert_eq!("Success", answer["response"]["result"]["status"]);
    assert_eq!(json!({"apiVersion": "example.com/v2", "kind": "Widget", "metadata": {"name": "a"}, "spec": {"replicas": 3, "size": null}}), answer["response"]["convertedObjects"][0]);
    let failing = json!({"apiVersion": "apiextensions.k8s.io/v1", "kind": "ConversionReview", "request": {"uid": "u2", "desiredAPIVersion": "example.com/v9", "objects": [{"apiVersion": "example.com/v1", "kind": "Widget"}]}});
    let (_, failed) = post(&client, "/convert", &failing.to_string());
    assert_eq!("Failure", failed["response"]["result"]["status"]);
    assert_eq!("can't convert to example.com/v9", failed["response"]["result"]["message"]);
}

#[test]
fn bad_requests_and_other_paths() {
    let client = TestClient::new(webhooks().wrap(rws_server::app::App::with_state(()).get("/other", |_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "app"))));
    assert_eq!(400, client.post("/validate").body_json("{\"kind\":\"nope\"}").send().status());
    assert_eq!(400, client.post("/convert").body_json("not json").send().status());
    assert_eq!(405, client.get("/validate").send().status());
    assert_eq!("app", client.get("/other").send().body_text());
}

#[test]
fn certificates_cover_the_service_names() {
    let cert = WebhookCertificate::generate("op", "ops").unwrap();
    assert!(cert.ca_pem.starts_with("-----BEGIN CERTIFICATE-----") && cert.key_pem.contains("PRIVATE KEY"));
    assert_ne!(cert.ca_pem, cert.cert_pem);
    // The serving certificate validates against the CA for op.ops.svc.
    use rustls_pki_types::pem::PemObject;
    let ca = rustls_pki_types::CertificateDer::from_pem_slice(cert.ca_pem.as_bytes()).unwrap();
    let leaf = rustls_pki_types::CertificateDer::from_pem_slice(cert.cert_pem.as_bytes()).unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca).unwrap();
    let verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(std::sync::Arc::new(roots), std::sync::Arc::new(rustls::crypto::aws_lc_rs::default_provider())).build().unwrap();
    use rustls::client::danger::ServerCertVerifier;
    let name = rustls_pki_types::ServerName::try_from("op.ops.svc").unwrap();
    verifier.verify_server_cert(&leaf, &[], &name, &[], rustls_pki_types::UnixTime::now()).unwrap();
    let other = rustls_pki_types::ServerName::try_from("other.ops.svc").unwrap();
    assert!(verifier.verify_server_cert(&leaf, &[], &other, &[], rustls_pki_types::UnixTime::now()).is_err());
}

fn base64_decode(text: &str) -> Vec<u8> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for c in text.bytes().filter(|c| *c != b'=') {
        buf = buf << 6 | T.iter().position(|t| *t == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}
