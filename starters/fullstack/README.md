# {{project-name}}

A full-stack app on [rust-web-server](https://rws8.tech): a lit frontend, a
JSON API and PostgreSQL, deployed to a [kube-dc](https://kube-dc.com) project
by GitHub Actions.

```sh
(cd frontend && npm ci && npm run build)
cargo test
cargo run          # http://127.0.0.1:7878 — notes kept in memory without PGHOST
```

Frontend with live reload: `cargo run` in one terminal, `cd frontend && npm run dev`
in another (http://127.0.0.1:5173, `/api` proxied to the app).

## Deploy to kube-dc

1. **Project.** Create a kube-dc project for the app (and one for previews,
   e.g. `previews`), or use existing ones. The namespace is
   `<organization>-<project>`.
2. **Database (optional).** In the console or with the kube-dc MCP tool
   `create_managed_service`, create a PostgreSQL named `<app>-db` (plan
   `postgresql-development`, parameter `database: <app>`). Its credentials
   arrive in Secret `<app>-db-owner`, which the app reads; without it notes
   are kept in memory. `<app>` is the repository name in lower case.
3. **Machine identity.** Organization → Access → Machine identities: create
   one in an Organization group with the `developer` role in both projects
   (`admin` if the image is private: the pull Secret needs it), and open
   **Show credentials**.
4. **GitHub settings** (Settings → Secrets and variables → Actions):

   | Kind | Name | Value |
   |---|---|---|
   | Variable | `KUBE_DC_API_SERVER` | Kubernetes API URL from the credentials |
   | Variable | `KUBE_DC_TOKEN_URL` | Token URL from the credentials |
   | Variable | `KUBE_DC_NAMESPACE` | e.g. `my-org-notes` |
   | Variable | `KUBE_DC_PREVIEW_NAMESPACE` | e.g. `my-org-previews` |
   | Secret | `KUBE_DC_CLIENT_ID`, `KUBE_DC_CLIENT_SECRET` | From the credentials |
   | Secret | `KUBE_DC_CA_CERT` | Only if the API server's certificate is not publicly trusted |
   | Secret | `GHCR_PULL_TOKEN` | Only for a private package: a token with `read:packages` |

5. **Push to `main`.** `deploy` tests, builds the image into GHCR, deploys
   and prints the HTTPS URL. Each pull request gets a preview at its own
   URL (`preview`), removed when the PR closes.

By hand: `KUBECONFIG=$(deploy/kubeconfig.sh) deploy/deploy.sh <namespace> <app> <image>`.
