//! Wiring only.
//!
//! ```sh
//! operator --manifests --image <image> [--namespace <ns>] | kubectl apply -f -
//! ```
//!
//! In the pod: make the webhooks' TLS certificate and put its CA into the
//! webhook configurations (`/readyz` waits for that), run the controllers
//! under leader election, and serve the webhooks, probes and metrics over
//! HTTPS.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rust_web_server::dev_mcp::DevMcpExt;
use rust_web_server::k8s::{client, manifests, webhook::WebhookCertificate};
use rust_web_server::server::Server;
use {{crate_name}}::{dev_tools, operator, server, NAME};

/// The value after `--<name>` on the command line.
fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == &format!("--{name}")).and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main]
async fn main() {
    let namespace = arg("namespace").unwrap_or_else(client::current_namespace);
    manifests::print_on_request(|| {
        let image = arg("image").unwrap_or_else(|| format!("{NAME}:latest"));
        {{crate_name}}::manifests(&namespace, &image, "")
    });

    let kube = client::client().await.expect("a Kubernetes client (in a pod, or ~/.kube/config)");
    // One certificate for every replica, kept in a Secret.
    let cert = WebhookCertificate::shared(&kube, &namespace, &format!("{NAME}-webhook-tls"), NAME).await.expect("the webhook certificate");
    let dir = std::env::var("WEBHOOK_TLS_DIR").unwrap_or_else(|_| "/tmp/webhook-tls".into());
    cert.install(&dir).expect("writing the webhook certificate");

    // The webhook configurations may not exist yet (applied with this
    // Deployment): keep trying, and stay not ready until it works.
    let ca_installed = Arc::new(AtomicBool::new(false));
    let installed = Arc::clone(&ca_installed);
    rust_web_server::health::readiness("webhook-ca", move || {
        if installed.load(Ordering::SeqCst) { Ok(()) } else { Err("the webhook CA isn't in the webhook configurations yet".into()) }
    });
    let patcher = kube.clone();
    tokio::spawn(async move {
        loop {
            match cert.patch_ca_bundle(&patcher, Some(NAME), Some(NAME), None).await {
                Ok(()) => {
                    ca_installed.store(true, Ordering::SeqCst);
                    break;
                }
                Err(e) => eprintln!("webhook CA: {e}; retrying"),
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    tokio::spawn(operator(kube, &namespace).run());
    let (listener, pool) = Server::setup().expect("server setup");
    Server::run_tls(listener, pool, server().dev_mcp_with(dev_tools())).await;
}
