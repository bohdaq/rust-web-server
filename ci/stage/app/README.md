# rws-e2e-app

The test app of the stage end-to-end suite (`ci/stage/`). The rws release
image has no library-only features (sessions, CSRF, asymmetric JWT, the ORM,
jobs, the mailer, S3 storage, OpenAPI, MCP, A2A, the OAuth server and
client), so this binary links rust-web-server by path and serves one route
per feature; the suites drive it with curl from outside the cluster.

A standalone crate (its own `[workspace]`, its own `Cargo.lock`), not part
of the repository workspace.

## Build and run

```sh
# Tests (no external service needed)
PATH=$HOME/.cargo/bin:$PATH CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test --manifest-path ci/stage/app/Cargo.toml

# Image, from the repository root (the crate depends on rws by path)
docker build -f ci/stage/app/Dockerfile -t rws-e2e-app:local .
docker run --rm -p 18080:8080 rws-e2e-app:local
curl localhost:18080/healthz
```

`Dockerfile.dockerignore` (next to the Dockerfile; BuildKit prefers it to a
root `.dockerignore`) limits the context to what the build needs. The image
is distroless `cc-debian12:nonroot`: the binary, `/app/migrations`,
`/app/templates`.

## Runtime

One binary, `rws-e2e-app`, plain HTTP on `0.0.0.0:8080` (TLS is the
gateway's; `RWS_CONFIG_IP`/`RWS_CONFIG_PORT` override). `ROLE` picks what it
serves: `app` (default), `idp` or `sp`. All configuration comes from the
environment. A feature whose variables are unset answers `503` with a body
`unavailable: <VAR> unset` (or the configuration error, e.g. an invalid PEM)
— the process never fails to start for a missing service. Startup problems
are logged to stderr with an `[e2e]` prefix. `GET /healthz` → `200 ok` in
every role.

## ROLE=app

| # | Route | Answer |
|---|---|---|
| | `GET /healthz` | `200` `ok` |
| | `GET /app/pod` | `200`, `$HOSTNAME` (which replica answered) |
| 1 | `POST /app/session/login?user=<name>` | `200` body `<name>`, `Set-Cookie: rws_e2e_sid=…`; `400` without `user` |
| 1 | `GET /app/session/me` | `200` body `<name>` with the cookie; `401` without or unknown |
| 1 | `GET /app/session/store` | `redis` (`SESSION_REDIS` set) or `memory` |
| 2 | `GET /app/csrf/form` | `200` HTML with `<input type="hidden" name="_csrf" value="<token>">` and `Set-Cookie: _csrf=<token>; Path=/; SameSite=Lax` |
| 2 | `POST /app/csrf/submit` | `200` `csrf-ok` when the `_csrf` cookie equals the `_csrf` form field (`Content-Type: application/x-www-form-urlencoded`) or the `X-CSRF-Token` header; `403` otherwise |
| 3 | `GET /app/jwt/rs256` | `Authorization: Bearer <RS256 JWT>` → `200` body = `sub`; `401` otherwise (bad signature, `alg` not RS256, expired `exp`, no `sub`) |
| 3 | `GET /app/jwt/es256` | the same with ES256 |
| 4 | `POST /app/webhook/github` | `X-Hub-Signature-256: sha256=<hex HMAC-SHA256(body, WEBHOOK_SECRET)>` → `200` `webhook-ok`; `401` otherwise |
| 5 | `GET /app/sse?count=N&every_ms=M` | `text/event-stream`: events `id: i` / `data: {"n":i}` for i = 1…N, M ms apart, then the stream ends (defaults N=3, M=100; N ≤ 10000, M ≤ 60000) |
| 6 | `GET /app/hello?name=x` | `200` `<p>Hello, x</p>` (Tera, HTML-escaped: `name=%3Cb%3E` → `<p>Hello, &lt;b&gt;</p>`) |
| 7 | `POST /app/todos` `{"title":"x"}` | `201` `{"id":1,"title":"x"}`; `400` on bad JSON |
| 7 | `GET /app/todos/:id` | `200` `{"id":…,"title":…}`; `404` |
| 7 | `DELETE /app/todos/:id` | `204` (also for an id that doesn't exist) |
| 8 | `POST /app/jobs?key=K[&fail_first=1]` | `202` `{"id":…,"key":"K","fail_first":false}` |
| 8 | `GET /app/jobs/K` | `200` `{"runs":[{"pod":"<HOSTNAME>","attempts":1,"at_ms":…}],"pending":0,"failed":0}` |
| 9 | `POST /app/mail?to=a@e2e.test&subject=S` | `200` `sent`: a plain-text mail from `rws-e2e@e2e.test`, subject `S`, body `rws-e2e-app test mail: S`; `502` on an SMTP error |
| 10 | `PUT /app/files/:name` (body) | `201` body `<name>` (`Content-Type` of the request is stored) |
| 10 | `GET /app/files/:name` | `200` the bytes; `404` when S3 says so |
| 10 | `GET /app/files/:name/presign` | `200` body = a presigned `GET` URL, valid 300 s |
| 10 | `DELETE /app/files/:name` | `204` |
| 11 | `GET /openapi.json`, `GET /docs` | OpenAPI 3.1 document (todos described with schemas), Swagger UI |
| 12 | `POST /mcp` | MCP Streamable HTTP, `Authorization: Bearer $MCP_TOKEN` (`401` without); tool `add`, resource `e2e://hello`, prompt `greet` |
| 13 | `GET /.well-known/agent-card.json`, `POST /a2a` | A2A 1.0 echo agent |
| 14 | `POST /v1/chat/completions`, `GET /v1/models` | mock OpenAI-compatible LLM, no auth |

Notes per feature:

- **Sessions** use rws's `session` module: `RedisSessionStore` when
  `SESSION_REDIS=host:port` (and `SESSION_REDIS_PASSWORD`, optional) — shared
  by every replica — else the in-memory `SessionStore` (per replica). rws has
  a `DbSessionStore` too, but it doesn't work on Postgres (see Findings).
- **CSRF** is rws's `CsrfLayer` (double-submit cookie), applied only under
  `/app/csrf/` — it checks every mutating request of the app it wraps.
  Field name `_csrf`, header `X-CSRF-Token`, cookie `_csrf`:

  ```sh
  curl -c jar localhost:8080/app/csrf/form
  T=$(awk '$6=="_csrf"{print $7}' jar)
  curl -b jar -d "_csrf=$T" localhost:8080/app/csrf/submit     # csrf-ok
  ```
- **JWT** keys are SubjectPublicKeyInfo PEMs (`BEGIN PUBLIC KEY`; RS256 also
  takes PKCS#1 `BEGIN RSA PUBLIC KEY`), 2048–8192-bit RSA. A one-line value
  with literal `\n` escapes is accepted. ES256 signatures are raw `r||s`
  (64 bytes), as JWS requires. Checked with `auth::verify_jwt_rs256` /
  `verify_jwt_es256`.
- **Jobs** (Postgres): every job writes one `job_runs` row
  `(job_key, pod = $HOSTNAME, attempts, at)` when it succeeds; claiming uses
  `FOR UPDATE SKIP LOCKED` and the row is written in the transaction that
  finishes the job, so across replicas each job succeeds exactly once
  (`runs` has one entry). Retry policy = rws's job queues' defaults: 3
  retries after the first attempt, 500 ms backoff doubled per retry.
  `fail_first=1` fails attempt 1, so the run row has `attempts: 2`, about
  0.5 s later. Two workers per replica poll every 200 ms. A job left
  `running` for 60 s (its pod died) is handed out again. Poll
  `GET /app/jobs/K` until `pending` is 0. This is the app's own queue, not
  rws's `PersistentJobQueue`, which fails on Postgres (Findings).
- **Database**: `RWS_DB_*` (rws's `DbConfig::from_env`): `RWS_DB_HOST`
  (`localhost`), `RWS_DB_PORT` (`5432`), `RWS_DB_USER`, `RWS_DB_PASSWORD`,
  `RWS_DB_NAME` (required), `RWS_DB_POOL_SIZE` (10), `RWS_DB_BACKEND`
  (defaults to `postgres` here). Connecting is retried for 60 s in the
  background (routes answer `503 database still connecting` meanwhile), then
  `migrations/` (`MIGRATIONS_DIR`, `/app/migrations` in the image) runs via
  `DbPool::migrate` (retried: replicas race). Tables: `todos`
  (`#[derive(Model)]`), `e2e_jobs`, `job_runs`, `_schema_migrations`. After
  that failure, DB routes answer `503 database unavailable: …` until restart.
- **S3**: `S3Storage` (path-style `{endpoint}/{bucket}/{key}`; rws offers no
  virtual-hosted mode). The presigned URL is SigV4 query signing
  (`UNSIGNED-PAYLOAD`, only `host` signed) done by this app — rws has none.
  **Use an endpoint on the scheme's default port** (`https://host`, not
  `https://host:9000`): rws signs requests for the host without the port.
- **MCP** with curl: `initialize` first, then send its `Mcp-Session-Id`:

  ```sh
  H=(-H "Authorization: Bearer $MCP_TOKEN" -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream')
  SID=$(curl -si "${H[@]}" localhost:8080/mcp \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"curl","version":"1"}}}' \
    | awk -F': ' 'tolower($1)=="mcp-session-id"{print $2}' | tr -d '\r')
  curl -s "${H[@]}" -H "Mcp-Session-Id: $SID" localhost:8080/mcp \
    -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"add","arguments":{"a":2,"b":40}}}'
  # → …"content":[{"type":"text","text":"42"}]…
  ```
  `resources/read` `{"uri":"e2e://hello"}` → `hello from rws-e2e-app`;
  `prompts/get` `{"name":"greet","arguments":{"name":"Ann"}}` → a user
  message `Say hello to Ann.`. Without `MCP_TOKEN` every `/mcp` request is
  `503`.
- **A2A**: the card is at the root, `GET /.well-known/agent-card.json`
  (rws's fixed path; its `url` comes from the `Host` header, or
  `A2A_PUBLIC_URL`). JSON-RPC at `POST /a2a` with header `A2A-Version: 1.0`:

  ```sh
  curl -s -H 'A2A-Version: 1.0' -H 'Content-Type: application/json' localhost:8080/a2a \
    -d '{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"m1","role":"ROLE_USER","parts":[{"text":"ping"}]}}}'
  # → {"jsonrpc":"2.0","id":1,"result":{"message":{…,"role":"ROLE_AGENT","parts":[{"text":"ping"}]}}}
  ```
- **LLM mock**: content `echo: <last user message>` (string content, or the
  text parts of an array), `usage` `{prompt_tokens:10, completion_tokens:5,
  total_tokens:15}`, the request's `model` echoed (default `mock-1`). With
  `"stream": true`: `data:` chunks (`chat.completion.chunk`: the role, the
  content in two pieces, then `finish_reason: "stop"` with `usage`), then
  `data: [DONE]`. `400` for invalid JSON or no user message.

## ROLE=idp

rws's `AuthServer` as the issuer `ISSUER` (the public https URL, no trailing
slash) with one client: `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`,
`OIDC_REDIRECT_URI` (grants `authorization_code`, `refresh_token`,
`client_credentials`; scopes `openid profile email`); tokens HS256-signed
with `IDP_SIGNING_SECRET`. Run **one replica**: codes and refresh tokens are
in memory.

| Route | |
|---|---|
| `GET /.well-known/openid-configuration`, `GET /.well-known/jwks.json` | rws's discovery document; JWKS is always `{"keys":[]}` |
| `GET /oauth/authorize` | rws: without a session → `302 /login?return_to=<this URL, encoded>`; with one → `302 <redirect_uri>?code=…&state=…` |
| `POST /oauth/token` | rws (form body; client credentials in the form, not HTTP Basic) |
| `GET /login?user=<name>&return_to=<path>` | this app: puts `user_id=<name>` in the session `AuthServer` reads (cookie `_rws_authz_sid`) and redirects to `return_to` (a path only; `400` otherwise) |
| `GET /userinfo` | this app: `Bearer <access token>` (HS256-verified) → `{"sub","iss","scope"}`; `401` |

The code flow with curl:

```sh
I=https://idp.example; R=https://sp.example/auth/callback
L=$(curl -s -o /dev/null -w '%{redirect_url}' "$I/oauth/authorize?response_type=code&client_id=$ID&redirect_uri=$R&state=s1")
# L = $I/login?return_to=%2Foauth%2Fauthorize%3F…
L=$(curl -s -c idp.jar -o /dev/null -w '%{redirect_url}' "$L&user=alice")   # back to /oauth/authorize
L=$(curl -s -b idp.jar -o /dev/null -w '%{redirect_url}' "$L")              # $R?code=…&state=s1
CODE=$(sed 's/.*code=\([^&]*\).*/\1/' <<<"$L")
curl -s "$I/oauth/token" -d grant_type=authorization_code -d code=$CODE \
  -d client_id=$ID -d client_secret=$SECRET --data-urlencode redirect_uri=$R
# → {"access_token":"…","token_type":"Bearer","expires_in":3600,"refresh_token":"…","id_token":"…"}
curl -s -H "Authorization: Bearer $ACCESS" $I/userinfo   # {"sub":"alice",…}
```

The `id_token` is the access token (HS256, claims `sub iss aud exp iat
scope`, no `nonce`). PKCE (`code_challenge`, S256) is optional.

## ROLE=sp

rws's `OidcAuth` against `ISSUER` with the same `OIDC_CLIENT_ID` /
`OIDC_CLIENT_SECRET`; `OIDC_REDIRECT_URI` is its own callback (its path
becomes the callback path, normally `/auth/callback`). Sessions live in
HMAC-signed cookies (`SP_COOKIE_SECRET`, default the client secret), so any
number of replicas work.

| Route | |
|---|---|
| `GET /sp/me` | signed in → `200` `{"sub":"alice","email":null,"name":null,"verified_by":"userinfo"}`; else `302 /auth/login` |
| `GET /auth/login` | rws → `302 $ISSUER/oauth/authorize?…` |
| `GET <callback path>` | rws: checks `state`, exchanges the code, → `302 /sp/me` (the original page) |
| `GET /auth/logout` | rws |

`SP_VERIFY` picks how the SP learns who signed in:

- `userinfo` (default, works): the provider is configured by hand with no
  `jwks_uri`, so `OidcAuth` takes its OAuth-only path (no PKCE, no
  `id_token` check) and asks the IdP's `/userinfo` (served by this app's
  IdP role — `AuthServer` has none).
- `jwks`: the standard OIDC path — `OidcProvider::discover(ISSUER)`, then
  the `id_token` verified against the JWKS. **Fails**: the callback answers
  `500 {"detail":"JWT signature verification failed"}`, because `AuthServer`
  signs HS256 and publishes no keys, and `JwksCache` verifies RS256/ES256
  only. (Its `id_token` has no `nonce` either, which `OidcAuth` checks next.)

Full flow with curl (cookie jars for both hosts):

```sh
L=$(curl -s -c sp.jar -o /dev/null -w '%{redirect_url}' $SP/sp/me)            # /auth/login
L=$(curl -s -b sp.jar -c sp.jar -o /dev/null -w '%{redirect_url}' "$L")       # IdP authorize
L=$(curl -s -o /dev/null -w '%{redirect_url}' "$L")                           # IdP /login
L=$(curl -s -c idp.jar -o /dev/null -w '%{redirect_url}' "$L&user=alice")     # IdP authorize
L=$(curl -s -b idp.jar -o /dev/null -w '%{redirect_url}' "$L")                # SP callback
curl -s -b sp.jar -c sp.jar -o /dev/null "$L"                                  # 302 /sp/me
curl -s -b sp.jar $SP/sp/me                                                    # {"sub":"alice",…}
```

## Environment

| Variable | Role | Used for |
|---|---|---|
| `ROLE` | all | `app` (default), `idp`, `sp` |
| `RWS_CONFIG_IP`, `RWS_CONFIG_PORT` | all | listen address (`0.0.0.0`, `8080`); any other `RWS_CONFIG_*` works too |
| `HOSTNAME` | app | the pod name in `job_runs` and `/app/pod` |
| `SESSION_REDIS`, `SESSION_REDIS_PASSWORD` | app | Redis session store (`host:port`) |
| `JWT_RS256_PUBLIC_KEY`, `JWT_ES256_PUBLIC_KEY` | app | PEM public keys |
| `WEBHOOK_SECRET` | app | GitHub webhook HMAC secret |
| `RWS_DB_*`, `MIGRATIONS_DIR` | app | Postgres, migrations directory |
| `SMTP_HOST`, `SMTP_PORT` (25) | app | plain SMTP, no TLS, no auth |
| `S3_ENDPOINT`, `S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `S3_REGION` (`us-east-1`) | app | S3 |
| `MCP_TOKEN` | app | bearer token for `/mcp` |
| `A2A_PUBLIC_URL` | app | the agent card's `url` (else from `Host`) |
| `ISSUER`, `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `OIDC_REDIRECT_URI` | idp, sp | OAuth/OIDC |
| `IDP_SIGNING_SECRET` | idp | HS256 signing secret |
| `SP_COOKIE_SECRET`, `SP_VERIFY` | sp | session cookie HMAC key; `userinfo` / `jwks` |

The SP calls `ISSUER` with rws's HTTP client, which trusts the WebPKI roots
only: the IdP's certificate must chain to a public CA.

## Findings (rws gaps this app works around)

- `jobs::PersistentJobQueue` and `session::DbSessionStore` write `?`
  placeholders; on Postgres `enqueue` fails with `syntax error at or near ","`
  and the worker's poll with `syntax error at or near "ORDER"`.
- On Postgres, `DbPool::query_rows` decodes `INT4`, `NUMERIC` and
  `TIMESTAMPTZ` columns as `NULL` (`get` → `unexpected NULL`), so `INTEGER`
  / `SERIAL` columns can't be read: the migrations here use `BIGINT` only.
- `DbPool::migrate` takes no lock: replicas starting together fail with
  `duplicate key value violates unique constraint "pg_type_typname_nsp_index"`.
- `S3Storage` signs the host without its port while the client sends
  `Host: host:port`: any endpoint on a non-default port gets
  `SignatureDoesNotMatch`. No presigned URLs.
- `extract::Query` (`URL::parse_query`) decodes only some upper-case escapes,
  decodes `%25` first (`%2526` → `&`) and cuts values at a second `=`; this
  app decodes queries itself.
- `AuthServer` (HS256, empty JWKS, no `nonce`, no userinfo endpoint) and
  `OidcAuth` (JWKS RS256/ES256) don't interoperate (above); the authorize
  session cookie `_rws_authz_sid` is a private const.
- `CsrfLayer` has no path scope; it is wrapped here.
