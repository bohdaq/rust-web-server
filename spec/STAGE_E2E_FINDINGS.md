[Read Me](../README.md) > [Spec](.) > Stage E2E findings

# Stage E2E findings

What the stage end-to-end suite (`ci/stage/`, design: the "rws on kube-dc
stage — E2E test design" doc) found, one row per finding, keyed by case id so
a rerun shows what is fixed. Kinds: **bug** (wrong behaviour), **gap**
(documented or expected but missing), **incomplete** (partly works),
**blocked** (a kube-dc fault, not rws).

| Case | Kind | Finding | Status |
|---|---|---|---|
| H0 chart config | gap | The Helm chart had no way to give rws a `rws.config.toml`: no ConfigMap mount, no extra volumes, so a chart user couldn't configure routes, upstreams or middleware at all (env vars only). | fixed: `configFile`, `configMountPath`, `extraVolumes`, `extraVolumeMounts` (unreleased) |
