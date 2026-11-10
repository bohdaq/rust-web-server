---
title: Operators
description: Controllers with reconcile, requeue, finalizers and status; admission and conversion webhooks; CRDs and RBAC from Rust types; tests against a real API server.
---

An operator watches custom resources and makes the cluster match them. To start from a working one — a custom resource, a controller, webhooks, `--manifests`, tests — generate the starter:

```sh
cargo generate --git https://github.com/bohdaq/rust-web-server starters/operator
```

 The `k8s` feature (18.36.0) has the parts on top of [kube-rs](https://kube.rs)'s controller runtime: controllers with finalizers and status, an operator that runs them under leader election with metrics and a readiness check, admission and conversion webhooks with their certificate, CRD and RBAC manifests generated from the code, and tests against a real API server.

## A custom resource

A resource's types are Rust structs; `#[derive(CustomResource)]` (kube) makes the kind and its CRD:

```rust
use rust_web_server::k8s::{kube::CustomResource, schemars::JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "example.com", version = "v1", kind = "Backup", namespaced, status = "BackupStatus")]
pub struct BackupSpec {
    pub schedule: String,
    pub database: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
pub struct BackupStatus {
    pub last_run: Option<String>,
}
```

## Controllers

```rust
use std::sync::Arc;
use std::time::Duration;
use rust_web_server::k8s::controller::{Action, Context, Controller, Operator};
use rust_web_server::k8s::kube::api::Api;
use rust_web_server::k8s::Error;
use k8s_openapi::api::batch::v1::CronJob;

async fn reconcile(backup: Arc<Backup>, ctx: Arc<Context>) -> Result<Action, Error> {
    ctx.apply_owned(&*backup, cron_job_for(&backup)).await?;                 // owned: deleted with it
    ctx.patch_status(&*backup, serde_json::json!({ "last_run": null })).await?;
    ctx.event(&*backup, false, "Scheduled", "the CronJob is up to date").await?;
    Ok(Action::requeue(Duration::from_secs(300)))
}

let kube = rust_web_server::k8s::client::client().await?;
Operator::new(kube.clone(), "backup-operator")
    .leader_election(&rust_web_server::k8s::client::current_namespace(), "backup-operator")
    .controller(
        Controller::new(Api::<Backup>::all(kube.clone()))
            .owns(Api::<CronJob>::all(kube.clone()))          // a change to its CronJob reconciles the Backup
            .reconcile(reconcile)
            .finalizer("example.com/cleanup", |backup, ctx| async move { delete_backups_of(&backup).await }),
    )
    .run()
    .await;
```

| `Controller` | What |
|---|---|
| `new(api)` | Watch `api`'s objects (`Api::all` or `Api::namespaced`) |
| `watching(config)` | Only those a label or field selector picks |
| `owns(api)` | Also reconcile when an object it owns (owner reference) changes |
| `reconcile(f)` | `async fn(Arc<K>, Arc<Context>) -> Result<Action, Error>`: make the world match, answer when to look again (`Action::requeue(d)`, `Action::await_change()`) |
| `finalizer(name, cleanup)` | Add the finalizer to each object; on deletion run `cleanup`, then remove it |
| `on_error(delay)` | First retry delay after a failed reconcile (5 s), doubling per failure of the same object up to 5 min |
| `rule(rule)` | An extra RBAC rule the reconcile needs |

`Context` has the `client` and helpers: `patch_status(obj, json)` (merge into the status subresource), `apply_owned(owner, child)` (server-side apply with an owner reference, field manager = the operator's name) and `event(obj, warning, reason, note)` (a Kubernetes Event, shown by `kubectl describe`).

### The operator

`Operator::run` runs every controller and registers a readiness check (`/readyz`, see [health checks](/deployment/kubernetes/#health-checks)). Serve it beside the probes:

```rust
let operator = tokio::spawn(operator.run());
let (listener, pool) = Server::setup()?;
Server::run_tls(listener, pool, App::new()).await;     // /livez, /readyz, /metrics
```

`/metrics` gets `rws_controller_reconciles_total` and `rws_controller_reconcile_errors_total` per controller.

### Leader election

With `.leader_election(namespace, lease)` the controllers run only on the replica holding the Lease; the others wait. A replica that loses the Lease exits, and Kubernetes restarts it as a follower — kube's controllers can't be stopped and restarted in place, and an exit is the safe way to never reconcile on two replicas.

## Webhooks

`webhook::Webhooks` is an rws application answering the API server's AdmissionReview and ConversionReview requests:

```rust
use rust_web_server::k8s::webhook::{Webhooks, WebhookCertificate};

let webhooks = Webhooks::new()
    .validate::<Backup, _>("/validate-backups", |request| match &request.object {
        Some(b) if b.spec.schedule.is_empty() => Err("spec.schedule is required".into()),
        _ => Ok(()),
    })
    .mutate::<Backup, _>("/mutate-backups", |mut backup| {
        backup.labels_mut().insert("example.com/managed".into(), "true".into());
        Ok(backup)
    })
    .convert("/convert-backups", |object, desired_version| v1_to_v2(object, desired_version))
    .wrap(App::new());                                       // probes and metrics on the same server

// The API server only calls webhooks over TLS it trusts.
let cert = WebhookCertificate::shared(&kube, &namespace, "backup-operator-webhook-tls", "backup-operator").await?;
cert.install("/tmp/tls")?;                                    // RWS_CONFIG_TLS_CERT_FILE / KEY_FILE
cert.patch_ca_bundle(&kube, Some("backup-operator"), None, None).await?;
Server::run_tls(listener, pool, webhooks).await;
```

| Builder | The handler gets | Answers |
|---|---|---|
| `validate::<K, _>(path, f)` | The request: `operation`, `object`, `old_object`, `user_info`, `name`, `namespace` | `Ok(())` allows, `Err(message)` denies (kubectl shows the message) |
| `mutate::<K, _>(path, f)` | The object (on create and update) | The object changed; the JSON patch from old to new is computed for you |
| `convert(path, f)` | Each object and the desired `apiVersion` | The object in that version (`apiVersion` set for you) |

With more than one replica, use `WebhookCertificate::shared(&client, namespace, secret, service)`: the first replica makes the certificate and stores it in a `kubernetes.io/tls` Secret, the others read it (a create race is settled by the API server) — replicas each making their own would overwrite each other's `caBundle`, and the API server would reject every replica but one. The pod needs `get` and `create` on `secrets`; `to_secret`/`from_secret` convert (cert-manager's Secrets have the same keys).

`WebhookCertificate::generate(service, namespace)` makes a CA and a serving certificate for `<service>.<namespace>.svc` (`generate_for(name, dns_names)` for others); `install(dir)` writes `tls.crt`, `tls.key`, `ca.crt` and points rws's TLS settings at them; `patch_ca_bundle` puts the CA into the webhook configurations (and a CRD's conversion webhook) at startup, so no cert-manager is needed. With cert-manager, mount its Secret and set the TLS settings instead.

## Manifests

The CRDs, RBAC and webhook configurations come from the code, so they never drift from it:

```rust
use rust_web_server::k8s::manifests::{self, Rbac, WebhookRule, WebhookTarget};

manifests::print_on_request(|| {                // `backup-operator --manifests | kubectl apply -f -`
    let rbac = Rbac::new("backup-operator", "backups").rules(operator.rules());
    let target = WebhookTarget { service: "backup-operator".into(), namespace: "backups".into(), port: 443, path: "/validate-backups".into(), ca_pem: ca.clone() };
    let mut docs = vec![manifests::crd::<Backup>()];
    docs.extend(rbac.documents());
    docs.push(manifests::webhook_configuration("backups", false, &target, &[WebhookRule::new("example.com", "v1", "backups", &["CREATE", "UPDATE"])], true));
    docs
});
```

| Function | Writes |
|---|---|
| `crd::<K>()` | The CustomResourceDefinition from the type's schema |
| `with_conversion_webhook(crd, target)` | The CRD with `conversion.strategy: Webhook` |
| `Rbac::new(name, ns).rules(…).documents()` | ServiceAccount, ClusterRole (rules merged per resource), ClusterRoleBinding; `.leader_election()` adds the Lease rule |
| `Operator::rules()` | What its controllers need: their kinds and status, what they own, events, the Lease |
| `webhook_configuration(name, mutating, target, rules, fail_closed)` | A Validating/MutatingWebhookConfiguration |
| `to_yaml(&docs)` | A YAML stream: each document as JSON (which YAML accepts) after a `---` line |

## Testing against a real API server

A fake API server doesn't run admission, garbage collection, finalizers or status subresources — the things operators get wrong. `testing::TestCluster` uses a real one (kind, k3d, or envtest's `kube-apiserver`) from `RWS_TEST_KUBECONFIG`, with a namespace of its own per test:

```rust
use rust_web_server::k8s::testing::{eventually, TestCluster};

#[tokio::test]
async fn backups_get_a_cron_job() {
    let Some(cluster) = TestCluster::from_env().await else { return };   // skipped without a cluster
    cluster.install_crd::<Backup>().await.unwrap();                       // applied, then waited for
    let ns = cluster.namespace("backup").await.unwrap();                  // backup-<random>, labelled rws-test=true
    tokio::spawn(operator(cluster.client.clone()).run());
    ns.api::<Backup>().create(&Default::default(), &Backup::new("nightly", spec())).await.unwrap();
    let job = eventually(Duration::from_secs(30), || async { ns.api::<CronJob>().get_opt("nightly").await.ok().flatten() }).await;
    assert!(job.is_some());
    ns.delete().await;
}
```

```sh
kind create cluster --name test --kubeconfig /tmp/kubeconfig
RWS_TEST_KUBECONFIG=/tmp/kubeconfig cargo test
```

With `RWS_TEST_REQUIRE_KUBERNETES` set, a missing cluster fails the test instead of skipping it — for CI. rws's own CI runs its Kubernetes tests this way on kind, and deploys the operator starter into the cluster (`ci/e2e/operator.sh`): the API server calls its webhooks over TLS, its controller reconciles, one of two replicas leads. Never point it at a cluster that matters: the tests create and delete namespaces and cluster-wide CRDs.
