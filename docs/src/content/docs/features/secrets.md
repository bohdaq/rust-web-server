---
title: Secrets Manager Integration
description: Resolve vault://, aws-sm://, and azkv:// secret references to their live values at startup — HashiCorp Vault, AWS Secrets Manager, and Azure Key Vault, with no vendor SDK.
---

JWT signing keys, DB credentials, and TLS keys are ordinarily just plain environment variables. On a managed cloud, the idiomatic pattern is a managed secrets store — HashiCorp Vault, AWS Secrets Manager, or Azure Key Vault — rather than an init container that bridges secrets into env vars by hand.

The `secrets` module gives any config value that ability directly: a value of the form `vault://path#field`, `aws-sm://name`, or `azkv://vault-name/secret-name` is resolved over HTTP(S) at startup. A value that doesn't match one of these prefixes passes through unchanged — this is purely additive to every existing `RWS_CONFIG_*`/`RWS_*` environment variable, not a breaking change.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["secrets"] }
```

```rust,no_run
use rust_web_server::secrets;

// VAULT_ADDR / VAULT_TOKEN must already be set in the environment.
let db_password = secrets::resolve("vault://secret/myapp/db#password")?;
# Ok::<(), rust_web_server::secrets::SecretsError>(())
```

| Prefix | Backend | Example |
|---|---|---|
| `vault://path#field` | HashiCorp Vault (KV v2) | `vault://secret/myapp/db#password` |
| `aws-sm://name` or `aws-sm://name#field` | AWS Secrets Manager | `aws-sm://prod/db-password` |
| `azkv://vault-name/secret-name` | Azure Key Vault | `azkv://my-kv/db-password` |

## Automatic env var resolution

The most common way to use this feature is to never call `secrets::resolve` directly at all. Once the `secrets` feature is enabled, `Server::setup()` calls `secrets::resolve_env_vars()` automatically at startup: it scans every currently-set environment variable whose name starts with `RWS_`, and rewrites in place any whose *value* matches one of the prefixes above.

That means any `RWS_CONFIG_*` value — `RWS_CONFIG_TLS_KEY_FILE`, a config-driven proxy route's `token_env`-referenced variable, anything — can be a secret reference with **no code changes**:

```bash
export VAULT_ADDR=https://vault.internal:8200
export VAULT_TOKEN=s.xxxxxxxxxxxxxxxx
export RWS_CONFIG_TLS_KEY_FILE="vault://secret/myapp/tls#key"
rws
```

`resolve_env_vars()` is also exported publicly for a library user driving their own startup sequence instead of calling `Server::setup()`.

## Fail fast, not fail open

A value that *looks* like a secret reference but fails to resolve — wrong token, unreachable backend, missing field — is a hard startup error, not a silently-ignored one. `Server::setup()` returns `Err`, and the binary's `.expect("server setup failed")` panics rather than starting:

```
thread 'main' panicked at src/main.rs:439:
server setup failed: "resolving secret reference for RWS_CONFIG_LOG_FORMAT: Vault request to
http://127.0.0.1:1/v1/secret/data/rws/log_format failed: connect to '127.0.0.1:1' failed:
Connection refused (os error 61)"
```

The alternative — silently falling back to the literal, unresolved string — would mean a server starting up with a JWT signing key or database password literally equal to `"vault://secret/myapp/db#password"`, which is worse than refusing to start at all.

## HashiCorp Vault (KV v2)

```bash
vault kv put secret/myapp/db password=hunter2
```

```rust,no_run
use rust_web_server::secrets;

let password = secrets::resolve("vault://secret/myapp/db#password")?;
# Ok::<(), rust_web_server::secrets::SecretsError>(())
```

`path` is the logical KV v2 path exactly as used by `vault kv get path` — the first segment is the mount name (`secret` above), and the rest is the secret's path within that mount. This mirrors the Vault CLI's own convention so you don't have to think in terms of the KV v2 HTTP API's own path shape (`/v1/{mount}/data/{path}`).

| Variable | Default |
|---|---|
| `VAULT_ADDR` | `http://127.0.0.1:8200` |
| `VAULT_TOKEN` | **(required)** |

Only token auth (a static `X-Vault-Token` header) is implemented — no AppRole, Kubernetes auth, or token renewal. A short-lived token minted by one of those methods can still be placed directly into `VAULT_TOKEN` by whatever process starts `rws`.

## AWS Secrets Manager

```rust,no_run
use rust_web_server::secrets;

// Whole secret string:
let password = secrets::resolve("aws-sm://prod/db-password")?;

// Secrets Manager conventionally stores {"username":"...","password":"..."}
// as one secret — #field parses SecretString as JSON and extracts one field:
let password = secrets::resolve("aws-sm://prod/db-creds#password")?;
# Ok::<(), rust_web_server::secrets::SecretsError>(())
```

This backend reuses the exact same AWS authentication story the [S3-compatible storage](/features/storage/) feature already implements — `storage-s3`'s SigV4 request signer and its IRSA/ECS/IMDSv2 workload-identity credential chain — rather than a second, separate copy to keep in sync.

| Variable | Default |
|---|---|
| `AWS_REGION` or `AWS_DEFAULT_REGION` | **(required)** |
| `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` | optional — falls back to workload identity when unset |
| `AWS_ENDPOINT_URL_SECRETSMANAGER` | `secretsmanager.{region}.amazonaws.com` |

When static keys are unset, credentials are auto-detected in the same order documented on the [storage page](/features/storage/#workload-identity-no-static-keys): EKS IRSA, then ECS task role, then EC2 IMDSv2 as a last resort.

`AWS_ENDPOINT_URL_SECRETSMANAGER` overrides the default host — this is the same environment variable name the official AWS SDKs use for LocalStack or VPC-endpoint overrides, reused here rather than an rws-specific name.

## Azure Key Vault

```rust,no_run
use rust_web_server::secrets;

let password = secrets::resolve("azkv://my-kv/db-password")?;
# Ok::<(), rust_web_server::secrets::SecretsError>(())
```

Two auth modes are tried in order:

1. **Service principal (client-credentials grant)** — used when all three of `AZURE_KEY_VAULT_TENANT_ID`, `AZURE_KEY_VAULT_CLIENT_ID`, and `AZURE_KEY_VAULT_CLIENT_SECRET` are set. This is a standalone Azure AD app registration, independent of the host's own identity.
2. **Managed Identity** — falls back to the exact same Managed Identity detection [Azure Blob Storage](/features/storage/#azure-blob-storage-storage-azure-feature) already implements: App Service/Container Apps (`IDENTITY_ENDPOINT` + `IDENTITY_HEADER`), or VM/AKS IMDS as a last resort — just requesting a token scoped to Key Vault's resource URI instead of Storage's.

| Variable | Purpose |
|---|---|
| `AZURE_KEY_VAULT_TENANT_ID` | Azure AD tenant ID (service principal mode) |
| `AZURE_KEY_VAULT_CLIENT_ID` | App registration client ID (service principal mode) |
| `AZURE_KEY_VAULT_CLIENT_SECRET` | App registration client secret (service principal mode) |
| `AZURE_KEY_VAULT_ENDPOINT_OVERRIDE` | Overrides `https://{vault-name}.vault.azure.net` — a Key Vault emulator, or tests |
| `AZURE_AD_LOGIN_ENDPOINT_OVERRIDE` | Overrides `https://login.microsoftonline.com` — tests only |

Then `GET https://{vault-name}.vault.azure.net/secrets/{secret-name}?api-version=7.4` with `Authorization: Bearer {token}`.

## No vendor SDK

Every HTTP call in this module goes through the crate's existing outbound `http_client::Client` — no AWS SDK, no Azure SDK, no Vault client library. JSON responses are parsed with the crate's hand-rolled `json_lite` parser (shared with `service_discovery`'s Consul/etcd backends), matching the same no-third-party-parsing philosophy applied everywhere else in this crate.
