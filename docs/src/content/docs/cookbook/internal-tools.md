---
title: "Cookbook: internal tools behind sign-in"
description: Put Grafana, Gitea, admin panels or a staging site behind Google, Microsoft, Okta or Keycloak sign-in — or behind Authelia / Authentik — with no code in the tools.
---

Sign-in with your identity provider in front of any app
([`type = "oidc"`](/features/auth/#sign-in-with-google-okta-keycloak-type--oidc)).
Here Grafana for the SRE group, signed in through Keycloak, with Grafana's
auth proxy reading the user rws passes on:

```toml
port = 443
http_redirect_port = 80

[acme]
domains = ["grafana.example.com", "staging.example.com"]
email   = "ops@example.com"

[[upstream]]
name = "grafana"
backends = ["127.0.0.1:3000"]

[[upstream]]
name = "staging"
backends = ["10.0.2.15:8080"]

[[route]]
name = "grafana"
[route.match]
host = "grafana.example.com"
path = "/*"
[route.action]
type = "proxy"
upstream = "grafana"
[route.middleware.auth]
type              = "oidc"
issuer            = "https://id.example.com/realms/main"
client_id         = "grafana-proxy"
client_secret_env = "GRAFANA_OIDC_SECRET"
cookie_secret_env = "OIDC_COOKIE_SECRET"
allowed_groups    = ["sre"]

# The staging site for everyone with a company Google account.
[[route]]
name = "staging"
[route.match]
host = "staging.example.com"
path = "/*"
[route.action]
type = "proxy"
upstream = "staging"
[route.middleware.auth]
type              = "oidc"
provider          = "google"
client_id         = "1234567890-abc.apps.googleusercontent.com"
client_secret_env = "GOOGLE_OIDC_SECRET"
cookie_secret_env = "OIDC_COOKIE_SECRET"
allowed_domains   = ["example.com"]
exclude           = ["/healthz"]
```

In `grafana.ini`: `[auth.proxy] enabled = true`, `header_name =
X-Auth-Request-Email`, `header_property = email`, `auto_sign_up = true`.
Register `https://grafana.example.com/auth/callback` (and the staging one)
as redirect URIs with the provider; put the secrets in the environment
(`systemctl edit rws` → `Environment=…`). One `OIDC_COOKIE_SECRET` on every
replica keeps users signed in whichever replica they reach.

Already running Authelia or Authentik? Let it decide
([`type = "forward"`](/features/auth/#an-auth-service-decides-type--forward)):

```toml
port = 443

[[upstream]]
name = "gitea"
backends = ["127.0.0.1:3001"]

[[route]]
name = "gitea"
[route.match]
host = "git.example.com"
path = "/*"
[route.action]
type = "proxy"
upstream = "gitea"
[route.middleware.auth]
type         = "forward"
url          = "http://127.0.0.1:9091/api/authz/forward-auth"
copy_headers = ["Remote-User", "Remote-Groups", "Remote-Email", "Remote-Name"]
```
