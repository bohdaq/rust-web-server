//! Admission and conversion webhooks as an rws application: validating
//! (allow or deny a create/update/delete), mutating (change the object; the
//! JSON patch is computed for you) and CRD conversion between versions —
//! plus the TLS certificate the API server needs to trust them.
//!
//! ```rust,no_run
//! use rws_k8s::k8s::webhook::{Webhooks, WebhookCertificate};
//! # #[derive(rws_k8s::k8s::kube::CustomResource, serde::Deserialize, serde::Serialize, Clone, Debug, rws_k8s::k8s::schemars::JsonSchema)]
//! # #[kube(group = "example.com", version = "v1", kind = "Backup", namespaced)]
//! # pub struct BackupSpec { pub schedule: String }
//!
//! let webhooks = Webhooks::new()
//!     .validate::<Backup, _>("/validate-backups", |request| {
//!         match &request.object {
//!             Some(b) if b.spec.schedule.is_empty() => Err("spec.schedule is required".into()),
//!             _ => Ok(()),
//!         }
//!     })
//!     .mutate::<Backup, _>("/mutate-backups", |mut backup| {
//!         backup.labels_mut().entry("backups.example.com/managed".into()).or_insert("true".into());
//!         Ok(backup)
//!     });
//!
//! // A certificate for <service>.<namespace>.svc, served by run_tls.
//! let cert = WebhookCertificate::generate("backup-operator", "backups").unwrap();
//! cert.install("/tmp/webhook-tls").unwrap();   // sets RWS_CONFIG_TLS_CERT_FILE / KEY_FILE
//! # use rws_k8s::k8s::kube::ResourceExt;
//! ```
//!
//! Requests to other paths fall through to the wrapped app (`.wrap(app)`),
//! so one server can answer webhooks, probes and metrics. A request that
//! isn't an `AdmissionReview` gets `400`; a handler's `Err` denies with
//! that message (`kubectl` shows it).

use std::sync::Arc;

use kube::core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview, Operation};
use kube::core::conversion::{ConversionRequest, ConversionResponse, ConversionReview};
use kube::core::Status;
use kube::Resource;
use serde::de::DeserializeOwned;
use serde::Serialize;

use rws_server::application::Application;
use rws_server::request::Request;
use rws_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rws_server::server::ConnectionInfo;

pub use kube::core::admission::Operation as AdmissionOperation;

type Handler = Arc<dyn Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync>;

/// Webhook routes, an [`Application`](rws_server::application::Application).
#[derive(Clone, Default)]
pub struct Webhooks {
    routes: Vec<(String, Handler)>,
    fallback: Option<Arc<dyn Application + Send + Sync>>,
}

fn json_response(body: Vec<u8>) -> Response {
    Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, body)
}

/// The AdmissionReview answer to `body` from `decide`, which gets the
/// request and answers `Ok(None)` (allow), `Ok(Some(patch))` (allow with a
/// patch) or `Err(message)` (deny).
pub fn admission<K, F>(body: &[u8], decide: F) -> Result<Vec<u8>, String>
where
    K: Resource + DeserializeOwned + Serialize + Clone,
    F: Fn(&AdmissionRequest<K>) -> Result<Option<json_patch::Patch>, String>,
{
    let review: AdmissionReview<K> = serde_json::from_slice(body).map_err(|e| format!("not an AdmissionReview: {e}"))?;
    let request: AdmissionRequest<K> = review.try_into().map_err(|_| "the AdmissionReview has no request".to_string())?;
    let response = match decide(&request) {
        Ok(None) => AdmissionResponse::from(&request),
        Ok(Some(patch)) => AdmissionResponse::from(&request).with_patch(patch).map_err(|e| e.to_string())?,
        Err(message) => AdmissionResponse::from(&request).deny(message),
    };
    // kube serializes the patch's bytes as a JSON array of numbers; the
    // API's wire form for bytes is base64, as Go's encoding/json writes it.
    let mut review = serde_json::to_value(response.into_review()).map_err(|e| e.to_string())?;
    if let Some(bytes) = review["response"]["patch"].as_array().map(|a| a.iter().filter_map(|b| b.as_u64().map(|b| b as u8)).collect::<Vec<u8>>()) {
        review["response"]["patch"] = serde_json::Value::String(super::manifests::base64(&bytes));
    }
    serde_json::to_vec(&review).map_err(|e| e.to_string())
}

/// The ConversionReview answer to `body`: each object converted to the
/// desired `apiVersion` by `convert(object, desired_api_version)`.
pub fn conversion<F>(body: &[u8], convert: F) -> Result<Vec<u8>, String>
where
    F: Fn(serde_json::Value, &str) -> Result<serde_json::Value, String>,
{
    let review: ConversionReview = serde_json::from_slice(body).map_err(|e| format!("not a ConversionReview: {e}"))?;
    let request = ConversionRequest::from_review(review).map_err(|_| "the ConversionReview has no request".to_string())?;
    let desired = request.desired_api_version.clone();
    let objects = request.objects.clone();
    let response = ConversionResponse::for_request(request);
    let mut converted = Vec::with_capacity(objects.len());
    let mut failure = None;
    for object in objects {
        match convert(object, &desired) {
            Ok(mut o) => {
                o["apiVersion"] = serde_json::Value::String(desired.clone());
                converted.push(o);
            }
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }
    let response = match failure {
        None => response.success(converted),
        Some(e) => response.failure(Status::failure(&e, "ConversionFailed")),
    };
    serde_json::to_vec(&response.into_review()).map_err(|e| e.to_string())
}

impl Webhooks {
    pub fn new() -> Self {
        Webhooks::default()
    }

    /// A validating webhook at `path`: `check` gets the request (its
    /// `operation`, `object`, `old_object`, `user_info`) and answers `Ok`
    /// to allow or `Err(message)` to deny.
    pub fn validate<K, F>(mut self, path: &str, check: F) -> Self
    where
        K: Resource + DeserializeOwned + Serialize + Clone + 'static,
        F: Fn(&AdmissionRequest<K>) -> Result<(), String> + Send + Sync + 'static,
    {
        self.routes.push((path.to_string(), Arc::new(move |body| admission::<K, _>(body, |r| check(r).map(|_| None)))));
        self
    }

    /// A mutating webhook at `path`: `change` gets the object (on create and
    /// update) and returns it changed; the JSON patch from the old to the
    /// new is what the API server applies. `Err` denies.
    pub fn mutate<K, F>(mut self, path: &str, change: F) -> Self
    where
        K: Resource + DeserializeOwned + Serialize + Clone + 'static,
        F: Fn(K) -> Result<K, String> + Send + Sync + 'static,
    {
        self.routes.push((
            path.to_string(),
            Arc::new(move |body| {
                admission::<K, _>(body, |request| {
                    let Some(object) = request.object.clone() else { return Ok(None) };
                    if !matches!(request.operation, Operation::Create | Operation::Update) {
                        return Ok(None);
                    }
                    let before = serde_json::to_value(&object).map_err(|e| e.to_string())?;
                    let after = serde_json::to_value(change(object)?).map_err(|e| e.to_string())?;
                    let patch = json_patch::diff(&before, &after);
                    Ok((!patch.0.is_empty()).then_some(patch))
                })
            }),
        ));
        self
    }

    /// A CRD conversion webhook at `path` (see
    /// [`manifests::with_conversion_webhook`](super::manifests::with_conversion_webhook)):
    /// `convert(object, desired_api_version)` returns the object in that
    /// version (its `apiVersion` is set for you).
    pub fn convert<F>(mut self, path: &str, convert: F) -> Self
    where
        F: Fn(serde_json::Value, &str) -> Result<serde_json::Value, String> + Send + Sync + 'static,
    {
        self.routes.push((path.to_string(), Arc::new(move |body| conversion(body, &convert))));
        self
    }

    /// Requests to other paths go to `app`.
    pub fn wrap(mut self, app: impl Application + Send + Sync + 'static) -> Self {
        self.fallback = Some(Arc::new(app));
        self
    }
}

impl Application for Webhooks {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let path = request.request_uri.split('?').next().unwrap_or("");
        if let Some((_, handler)) = self.routes.iter().find(|(p, _)| p == path) {
            if request.method != "POST" {
                return Ok(Response::text(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed, "POST an AdmissionReview or a ConversionReview"));
            }
            return Ok(match handler(&request.body) {
                Ok(body) => json_response(body),
                Err(message) => Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, &message),
            });
        }
        match &self.fallback {
            Some(app) => app.execute(request, connection),
            None => {
                use rws_server::core::New;
                rws_server::app::App::new().execute(request, connection)
            }
        }
    }
}

/// A CA and a serving certificate for `<service>.<namespace>.svc` (and the
/// short names): the API server checks the webhook's certificate against
/// the CA in the webhook configuration's `caBundle`.
#[derive(Clone)]
pub struct WebhookCertificate {
    pub ca_pem: String,
    pub cert_pem: String,
    pub key_pem: String,
}

impl WebhookCertificate {
    /// A new CA and a certificate signed by it for the Service `service`
    /// in `namespace`, valid for ten years.
    pub fn generate(service: &str, namespace: &str) -> Result<Self, String> {
        let names = vec![
            service.to_string(),
            format!("{service}.{namespace}"),
            format!("{service}.{namespace}.svc"),
            format!("{service}.{namespace}.svc.cluster.local"),
        ];
        Self::generate_for(&format!("{service}.{namespace}.svc"), names)
    }

    /// The same for other DNS names (a webhook the API server reaches by
    /// URL rather than through a Service).
    pub fn generate_for(common_name: &str, names: Vec<String>) -> Result<Self, String> {
        use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose};
        let ca_key = KeyPair::generate().map_err(|e| e.to_string())?;
        let mut ca_params = CertificateParams::new(Vec::<String>::new()).map_err(|e| e.to_string())?;
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.distinguished_name.push(DnType::CommonName, format!("{common_name} webhook CA"));
        ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        ten_years(&mut ca_params);
        let ca = ca_params.self_signed(&ca_key).map_err(|e| e.to_string())?;
        let issuer = rcgen::Issuer::new(ca_params, ca_key);
        let key = KeyPair::generate().map_err(|e| e.to_string())?;
        let mut params = CertificateParams::new(names).map_err(|e| e.to_string())?;
        params.distinguished_name.push(DnType::CommonName, common_name.to_string());
        ten_years(&mut params);
        let cert = params.signed_by(&key, &issuer).map_err(|e| e.to_string())?;
        Ok(WebhookCertificate { ca_pem: ca.pem(), cert_pem: cert.pem(), key_pem: key.serialize_pem() })
    }

    /// Write `tls.crt`, `tls.key` and `ca.crt` into `dir` and point rws's
    /// TLS settings at them (`RWS_CONFIG_TLS_CERT_FILE`/`KEY_FILE`), so
    /// `Server::run_tls` serves them.
    pub fn install(&self, dir: &str) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
        let path = |f: &str| format!("{}/{f}", dir.trim_end_matches('/'));
        std::fs::write(path("tls.crt"), &self.cert_pem).map_err(|e| e.to_string())?;
        std::fs::write(path("tls.key"), &self.key_pem).map_err(|e| e.to_string())?;
        std::fs::write(path("ca.crt"), &self.ca_pem).map_err(|e| e.to_string())?;
        // SAFETY of set_var: settings are read through ServerConfig snapshots.
        unsafe {
            std::env::set_var("RWS_CONFIG_TLS_CERT_FILE", path("tls.crt"));
            std::env::set_var("RWS_CONFIG_TLS_KEY_FILE", path("tls.key"));
        }
        rws_server::server_config::invalidate();
        Ok(())
    }

    /// Put the CA into the `caBundle` of every webhook in the
    /// Validating/MutatingWebhookConfiguration `name` (and a CRD's
    /// conversion webhook, when `crd` is given) — for an operator that
    /// makes its own certificate at startup.
    pub async fn patch_ca_bundle(&self, client: &kube::Client, validating: Option<&str>, mutating: Option<&str>, crd: Option<&str>) -> super::Result<()> {
        use k8s_openapi::api::admissionregistration::v1::{MutatingWebhookConfiguration, ValidatingWebhookConfiguration};
        use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
        use kube::api::{Api, Patch, PatchParams};
        let bundle = super::manifests::base64(self.ca_pem.as_bytes());
        let params = PatchParams::default();
        if let Some(name) = validating {
            let api: Api<ValidatingWebhookConfiguration> = Api::all(client.clone());
            let current = api.get(name).await?;
            let hooks: Vec<serde_json::Value> = current.webhooks.unwrap_or_default().iter().map(|w| serde_json::json!({ "name": w.name, "clientConfig": { "caBundle": bundle } })).collect();
            api.patch(name, &params, &Patch::Strategic(serde_json::json!({ "webhooks": hooks }))).await?;
        }
        if let Some(name) = mutating {
            let api: Api<MutatingWebhookConfiguration> = Api::all(client.clone());
            let current = api.get(name).await?;
            let hooks: Vec<serde_json::Value> = current.webhooks.unwrap_or_default().iter().map(|w| serde_json::json!({ "name": w.name, "clientConfig": { "caBundle": bundle } })).collect();
            api.patch(name, &params, &Patch::Strategic(serde_json::json!({ "webhooks": hooks }))).await?;
        }
        if let Some(name) = crd {
            let api: Api<CustomResourceDefinition> = Api::all(client.clone());
            let patch = serde_json::json!({ "spec": { "conversion": { "webhook": { "clientConfig": { "caBundle": bundle } } } } });
            api.patch(name, &params, &Patch::Merge(&patch)).await?;
        }
        Ok(())
    }
}

fn ten_years(params: &mut rcgen::CertificateParams) {
    let now = std::time::SystemTime::now();
    params.not_before = now.into();
    params.not_after = (now + std::time::Duration::from_secs(10 * 365 * 86_400)).into();
}

#[cfg(test)]
mod tests;
