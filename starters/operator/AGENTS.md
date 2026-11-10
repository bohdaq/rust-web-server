# AGENTS.md

A Kubernetes operator on [rust-web-server](https://rws8.tech) (rws), on
kube-rs. For coding agents working in this project.

## Layout

The conventional rws layout (`rws.app.toml`; an operator's "routes" are its
controllers):

- `src/main.rs`: wiring only — `--manifests`, the webhook certificate (one
  for all replicas, in a Secret; its CA put into the webhook configurations,
  `/readyz` waits for that), the controllers, the HTTPS server.
- `src/lib.rs`: `operator(client, namespace)` (the controllers, leader
  election), `server()` (webhooks + probes + metrics), `manifests(...)`
  (everything to apply), `dev_tools()`.
- `src/controllers/<resource>.rs`: one per custom resource — its types
  (`#[derive(CustomResource)]`: the CRD), `controller(client)`, the
  reconcile and the cleanup. A new resource is a new module, added in
  `controllers/mod.rs`, `operator()` and `manifests()` (its CRD).
- `src/webhooks.rs`: the admission webhooks (`/validate`, `/mutate`).
- `tests/webhooks.rs`: AdmissionReviews through `TestClient` (no socket);
  `tests/cluster.rs`: the controller against a real API server when
  `RWS_TEST_KUBECONFIG` names one (skipped otherwise); `tests/dev_mcp.rs`:
  the dev MCP endpoint (CI runs it).

## Commands

- `cargo test`: everything that needs no cluster.
- `RWS_TEST_KUBECONFIG=<kubeconfig of a kind or k3d cluster> cargo test`:
  also the controller against it. Never a cluster that matters: tests make
  and delete namespaces and install the CRD.
- `cargo run -- --manifests --image <image> --namespace <ns>`: the YAML to
  `kubectl apply -f -`.
- `RWS_ENV=dev cargo run` against a dev cluster (`~/.kube/config`): the
  operator from your machine; the webhooks can't be called then (the API
  server can't reach you), so test them with `tests/webhooks.rs`.

## rws's operator framework, in brief

Docs: https://rws8.tech/deployment/kubernetes-operators/, all docs in one
file: https://rws8.tech/llms-full.txt.

- `Controller::new(api).owns(child_api).reconcile(f).finalizer(name, cleanup)`;
  `reconcile(obj, ctx) -> Result<Action, Error>`: make the cluster match
  `obj`, answer `Action::requeue(d)`; `ctx.apply_owned(owner, child)`
  (server-side apply, owner reference: deleted with it),
  `ctx.patch_status(obj, json)`, `ctx.event(obj, warning, reason, note)`.
- `Operator::new(client, name).leader_election(ns, lease).controller(c)`;
  `operator.rules()` is the RBAC its controllers need — `manifests()` adds
  it, so a new `owns(...)` needs no YAML edits.
- `Webhooks::new().validate::<K, _>(path, f).mutate::<K, _>(path, f)`; a
  validating `Err(message)` is what `kubectl` shows.
- Reconciles must be idempotent: they run again on every change, every
  restart, and every requeue.
- Health: `rust_web_server::health::readiness(name, f)` adds to `/readyz`.
