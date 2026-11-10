//! `Greeting` (greetings.example.com/v1): each one gets a ConfigMap
//! `<name>-greeting` holding its message, owned by it (deleted with it),
//! and a status saying so. Change the group to your own domain.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::CustomResource;
use rust_web_server::k8s::controller::{Action, Context, Controller};
use rust_web_server::k8s::kube::api::Api;
use rust_web_server::k8s::kube::ResourceExt;
use rust_web_server::k8s::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a `Greeting` asks for.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[kube(group = "example.com", version = "v1", kind = "Greeting", namespaced, status = "GreetingStatus", shortname = "gr")]
#[kube(printcolumn = r#"{"name":"Message","type":"string","jsonPath":".spec.message"}"#)]
#[kube(printcolumn = r#"{"name":"Ready","type":"boolean","jsonPath":".status.ready"}"#)]
pub struct GreetingSpec {
    /// The text (1 to 1000 characters; the validating webhook checks).
    pub message: String,
}

/// What the controller reports.
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct GreetingStatus {
    /// The ConfigMap holding the message.
    pub config_map: Option<String>,
    pub ready: bool,
}

/// Removed (after `cleanup`) when a Greeting is deleted.
pub const FINALIZER: &str = "example.com/greeting-cleanup";

/// The ConfigMap a Greeting should have.
pub fn desired_config_map(greeting: &Greeting) -> ConfigMap {
    ConfigMap {
        metadata: ObjectMeta { name: Some(format!("{}-greeting", greeting.name_any())), namespace: greeting.namespace(), ..Default::default() },
        data: Some(BTreeMap::from([("message".to_string(), greeting.spec.message.clone())])),
        ..Default::default()
    }
}

/// Make the ConfigMap match, then report it. Runs again when the Greeting
/// or its ConfigMap changes, and every 5 minutes.
pub async fn reconcile(greeting: Arc<Greeting>, ctx: Arc<Context>) -> Result<Action, Error> {
    let config_map = ctx.apply_owned(&*greeting, desired_config_map(&greeting)).await?;
    let status = GreetingStatus { config_map: config_map.metadata.name, ready: true };
    if greeting.status.as_ref() != Some(&status) {
        ctx.patch_status(&*greeting, serde_json::to_value(&status).map_err(|e| Error::Other(e.to_string()))?).await?;
        let _ = ctx.event(&*greeting, false, "Written", "the ConfigMap holds the message").await;
    }
    Ok(Action::requeue(Duration::from_secs(300)))
}

/// Before a Greeting goes. Its ConfigMap goes with it (an owned object);
/// clean up anything outside the cluster here.
pub async fn cleanup(greeting: Arc<Greeting>, ctx: Arc<Context>) -> Result<(), Error> {
    let _ = ctx.event(&*greeting, false, "Deleting", "cleaned up").await;
    Ok(())
}

pub fn controller(client: rust_web_server::k8s::kube::Client) -> Controller<Greeting> {
    Controller::new(Api::<Greeting>::all(client.clone()))
        .owns(Api::<ConfigMap>::all(client))
        .reconcile(reconcile)
        .finalizer(FINALIZER, cleanup)
}
