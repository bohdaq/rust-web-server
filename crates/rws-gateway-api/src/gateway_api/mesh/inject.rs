//! Sidecar injection: a mutating admission webhook the controller serves
//! (over TLS, the certificate in `RWS_CONFIG_GATEWAY_API_MESH_WEBHOOK_CERT_FILE`
//! / `_KEY_FILE`). Pods created in the namespaces the
//! MutatingWebhookConfiguration selects get two init containers, first:
//!
//! - `rws-mesh-init` ([`InjectSettings::init_image`]): sets the pod's
//!   iptables rules — outbound TCP to the sidecar's port, except the
//!   sidecar's own (UID [`SIDECAR_UID`](super::sidecar::SIDECAR_UID)) and
//!   loopback. Runs as root with `NET_ADMIN` and `NET_RAW`, then exits.
//! - `rws-mesh`: rws in `mesh-sidecar` mode, a native sidecar
//!   (`restartPolicy: Always`, Kubernetes ≥ 1.29) — started before the
//!   app's containers and kept running beside them.
//!
//! A pod opts out with the annotation `mesh.rws8.tech/inject: "false"`;
//! `hostNetwork` pods and pods that already have the sidecar are left alone.
//! The webhook allows every request: injection never blocks a pod.

use serde_json::{json, Value};

use crate::application::Application;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

use super::sidecar::{OUTBOUND_PORT, SIDECAR_UID};

/// Where the webhook answers.
pub const PATH: &str = "/mesh/v1/inject";
/// The default port of the webhook.
pub const DEFAULT_PORT: u16 = 15017;
/// The opt-out annotation (`"false"`).
pub const INJECT_ANNOTATION: &str = "mesh.rws8.tech/inject";
/// Set on injected pods.
pub const INJECTED_ANNOTATION: &str = "mesh.rws8.tech/injected";
/// The sidecar container's name.
pub const SIDECAR_NAME: &str = "rws-mesh";
/// The init container's name.
pub const INIT_NAME: &str = "rws-mesh-init";
/// The sidecar's health port (its rws server, for the probes).
pub const SIDECAR_HEALTH_PORT: u16 = 15000;

/// What injected containers run.
#[derive(Clone, Debug)]
pub struct InjectSettings {
    /// The sidecar's image (the rws image).
    pub sidecar_image: String,
    /// The image setting the iptables rules.
    pub init_image: String,
    /// The controller's configuration endpoint, for the sidecar.
    pub config_url: String,
}

/// The JSON patch injecting the sidecar into `pod`, or `None` when the pod
/// is left as it is.
pub fn patch(pod: &Value, settings: &InjectSettings) -> Option<Vec<Value>> {
    let annotations = &pod["metadata"]["annotations"];
    if annotations[INJECT_ANNOTATION].as_str().is_some_and(|v| v.eq_ignore_ascii_case("false")) {
        return None;
    }
    if pod["spec"]["hostNetwork"].as_bool() == Some(true) {
        return None;
    }
    let named = |list: &Value, name: &str| list.as_array().is_some_and(|l| l.iter().any(|c| c["name"] == name));
    if named(&pod["spec"]["initContainers"], SIDECAR_NAME) || named(&pod["spec"]["containers"], SIDECAR_NAME) {
        return None;
    }
    let (init, sidecar) = (init_container(settings), sidecar_container(settings));
    let mut ops = Vec::new();
    if pod["spec"]["initContainers"].as_array().is_some_and(|l| !l.is_empty()) {
        // Insert at the front, sidecar first so the init container ends up before it.
        ops.push(json!({"op": "add", "path": "/spec/initContainers/0", "value": sidecar}));
        ops.push(json!({"op": "add", "path": "/spec/initContainers/0", "value": init}));
    } else {
        ops.push(json!({"op": "add", "path": "/spec/initContainers", "value": [init, sidecar]}));
    }
    if annotations.is_object() {
        ops.push(json!({"op": "add", "path": format!("/metadata/annotations/{}", pointer_escape(INJECTED_ANNOTATION)), "value": "true"}));
    } else {
        ops.push(json!({"op": "add", "path": "/metadata/annotations", "value": {INJECTED_ANNOTATION: "true"}}));
    }
    Some(ops)
}

/// A JSON Pointer token (RFC 6901): `~` → `~0`, `/` → `~1`.
fn pointer_escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn init_container(settings: &InjectSettings) -> Value {
    json!({
        "name": INIT_NAME,
        "image": settings.init_image,
        "env": [
            {"name": "RWS_MESH_OUTBOUND_PORT", "value": OUTBOUND_PORT.to_string()},
            {"name": "RWS_MESH_UID", "value": SIDECAR_UID.to_string()}
        ],
        "securityContext": {
            "runAsUser": 0,
            "runAsNonRoot": false,
            "allowPrivilegeEscalation": false,
            "capabilities": {"add": ["NET_ADMIN", "NET_RAW"], "drop": ["ALL"]}
        },
        "resources": {"requests": {"cpu": "10m", "memory": "16Mi"}, "limits": {"memory": "64Mi"}}
    })
}

fn sidecar_container(settings: &InjectSettings) -> Value {
    let probe = json!({"httpGet": {"path": "/healthz", "port": SIDECAR_HEALTH_PORT}, "periodSeconds": 1, "failureThreshold": 60});
    json!({
        "name": SIDECAR_NAME,
        "image": settings.sidecar_image,
        "restartPolicy": "Always",
        "env": [
            {"name": "RWS_CONFIG_GATEWAY_API_MODE", "value": "mesh-sidecar"},
            {"name": "RWS_CONFIG_GATEWAY_API_MESH_CONFIG_URL", "value": settings.config_url},
            {"name": "RWS_CONFIG_PORT", "value": SIDECAR_HEALTH_PORT.to_string()},
            {"name": "RWS_CONFIG_IP", "value": "0.0.0.0"},
            {"name": "RWS_CONFIG_LOG_FORMAT", "value": "off"},
            {"name": "POD_NAMESPACE", "valueFrom": {"fieldRef": {"fieldPath": "metadata.namespace"}}}
        ],
        "startupProbe": probe,
        "securityContext": {
            "runAsUser": SIDECAR_UID,
            "runAsGroup": SIDECAR_UID,
            "runAsNonRoot": true,
            "allowPrivilegeEscalation": false,
            "readOnlyRootFilesystem": true,
            "capabilities": {"drop": ["ALL"]}
        },
        "resources": {"requests": {"cpu": "10m", "memory": "32Mi"}, "limits": {"memory": "256Mi"}}
    })
}

/// Answer an `AdmissionReview` (`admission.k8s.io/v1`): allowed, with the
/// patch when the pod gets the sidecar. `Err` for a body that isn't one.
pub fn review(body: &[u8], settings: &InjectSettings) -> Result<Vec<u8>, String> {
    let review: Value = serde_json::from_slice(body).map_err(|e| format!("not an AdmissionReview: {e}"))?;
    let request = &review["request"];
    let uid = request["uid"].as_str().ok_or("an AdmissionReview without request.uid")?;
    let mut response = json!({"uid": uid, "allowed": true});
    let is_pod_create = request["kind"]["kind"] == "Pod" && request["operation"] == "CREATE";
    if is_pod_create {
        if let Some(ops) = patch(&request["object"], settings) {
            let patch = serde_json::to_vec(&ops).map_err(|e| e.to_string())?;
            response["patchType"] = json!("JSONPatch");
            response["patch"] = json!(crate::core::base64::Base64::encode(&patch).map_err(|e| e.to_string())?);
        }
    }
    let api_version = review["apiVersion"].as_str().unwrap_or("admission.k8s.io/v1");
    serde_json::to_vec(&json!({"apiVersion": api_version, "kind": "AdmissionReview", "response": response})).map_err(|e| e.to_string())
}

/// The webhook as an [`Application`].
#[derive(Clone)]
pub struct Injector {
    pub settings: InjectSettings,
}

impl Application for Injector {
    fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        if request.method != "POST" || request.request_uri.split('?').next() != Some(PATH) {
            return Ok(Response::text(STATUS_CODE_REASON_PHRASE.n404_not_found, "not found"));
        }
        Ok(match review(&request.body, &self.settings) {
            Ok(body) => {
                let mut response = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &String::from_utf8_lossy(&body));
                response.headers.retain(|h| !h.name.eq_ignore_ascii_case("content-type"));
                response.headers.push(crate::header::Header::new("Content-Type", "application/json"));
                response
            }
            Err(e) => Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, &e),
        })
    }
}

/// Serve the webhook over TLS on `port` until the runtime stops.
pub async fn serve(injector: Injector, port: u16, cert_file: String, key_file: String) {
    let acceptor = match crate::tls::create_tls_acceptor(&cert_file, &key_file) {
        Ok(acceptor) => acceptor,
        Err(e) => {
            eprintln!("[gateway-api] mesh: can't serve the injection webhook: {e}");
            return;
        }
    };
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("[gateway-api] mesh: can't serve the injection webhook on port {port}: {e}");
            return;
        }
    };
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            continue;
        };
        tokio::spawn(crate::server::serve_tls_connection(stream, peer, acceptor.clone(), injector.clone()));
    }
}
