# Stage end-to-end suite

rws on kube-dc stage, the way users run it: the Helm chart with 3 replicas
behind the shared gateway (kube-dc terminates TLS), real cluster networking,
three backends. One run creates a throwaway project, runs the suites, writes
a report and deletes the project. On demand, before releases.

```bash
ci/stage/run.sh                      # every suite, the chart's image
ci/stage/run.sh --suites S1,S3       # some
ci/stage/run.sh --image 18.27.0      # another image tag
ci/stage/run.sh --keep               # leave the project (prints the delete command)
```

Needs `kubectl`, `helm`, `curl` (with brotli and zstd), `openssl`,
`python3` and a stage kubeconfig (`KUBECONFIG`, default
`~/.kube/stage_config`).

| Path | What |
|---|---|
| `run.sh` | project, deploy, suites, report, cleanup |
| `lib.sh` | `check`, HTTP and cluster helpers, ConfigMap edits (`config_edit` / `config_wait`) |
| `deploy.sh`, `manifests/` | the static tree ConfigMap and the whoami backends |
| `config/rws.config.toml` | the config every suite runs against (the chart's `configFile`) |
| `suites/S*.sh` | one file per suite; each case is a `check <id> <what> <test>` |
| `known.yaml` | cases expected to fail, with the finding they come from |
| `report/<run>/` | `summary.md`, `results.tsv`, pod logs, events (git-ignored) |

A case reports `ok`, `FAIL`, `KNOWN` (listed in `known.yaml`) or `BLOCKED`
(a kube-dc fault, not rws). Findings go in
[`spec/STAGE_E2E_FINDINGS.md`](../../spec/STAGE_E2E_FINDINGS.md), keyed by
case id. Bulk requests are capped at 40 a second: the gateway is shared.
