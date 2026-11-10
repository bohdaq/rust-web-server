//! The admission webhooks the API server calls for Greetings: one
//! validates (`/validate`), one sets defaults (`/mutate`).

use rust_web_server::k8s::kube::core::admission::AdmissionRequest;
use rust_web_server::k8s::kube::ResourceExt;
use rust_web_server::k8s::webhook::Webhooks;

use crate::controllers::greeting::Greeting;

/// The label every Greeting gets.
pub const LABEL: &str = "example.com/greeted";

/// Refuse a Greeting without a message, or with one over 1000 characters.
pub fn validate(request: &AdmissionRequest<Greeting>) -> Result<(), String> {
    let Some(greeting) = &request.object else { return Ok(()) };
    let n = greeting.spec.message.trim().chars().count();
    if n == 0 {
        Err("spec.message is required".into())
    } else if n > 1000 {
        Err(format!("spec.message is {n} characters; at most 1000"))
    } else {
        Ok(())
    }
}

/// Label each Greeting (`example.com/greeted: "true"`).
pub fn set_defaults(mut greeting: Greeting) -> Result<Greeting, String> {
    greeting.labels_mut().entry(LABEL.to_string()).or_insert_with(|| "true".to_string());
    Ok(greeting)
}

pub fn webhooks() -> Webhooks {
    Webhooks::new().validate::<Greeting, _>("/validate", validate).mutate::<Greeting, _>("/mutate", set_defaults)
}
