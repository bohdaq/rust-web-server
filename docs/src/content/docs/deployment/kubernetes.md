---
title: Kubernetes
description: Deploy rust-web-server on Kubernetes with health probes, metrics scraping, graceful shutdown, and autoscaling.
---

## Health endpoints

`rust-web-server` exposes two dedicated health endpoints that map directly to Kubernetes probe semantics:

| Endpoint | Purpose | Probe type |
|---|---|---|
| `GET /healthz` | Always returns `200 OK` while the process is running | Liveness |
| `GET /readyz` | Returns `200 OK` when ready, `503` during startup and SIGTERM drain | Readiness |

The `SERVER_READY` flag is set to `true` after `Server::setup()` completes and cleared as soon as a shutdown signal is received, so Kubernetes stops routing traffic before in-flight requests finish draining.

## Graceful shutdown

When Kubernetes sends `SIGTERM`, the server:

1. Clears `SERVER_READY` — `/readyz` begins returning `503`, Kubernetes stops sending new traffic.
2. Stops accepting new TCP connections.
3. Waits for all in-flight requests to complete (thread-pool drain on `http1`; tokio task drain on `http2`/`http3`).
4. Exits cleanly.

Set `terminationGracePeriodSeconds` to at least `30` to give in-flight requests time to complete before Kubernetes sends `SIGKILL`.

:::tip[Helm chart]
The manifests below are also a Helm chart, published with every release (since 18.2.0):

```bash
kubectl create secret tls rws-tls --cert=tls.crt --key=tls.key
helm install my-rws oci://ghcr.io/bohdaq/charts/rws --version 18.2.0
```

It renders the same `Deployment`, `Service`, `PodDisruptionBudget` and `HorizontalPodAutoscaler`, plus an optional `Ingress` and prometheus-operator `ServiceMonitor`; CI installs it into a kind cluster with TLS on and off. Its values are documented in [`helm/rws/README.md`](https://github.com/bohdaq/rust-web-server/tree/main/helm/rws).
:::

## Deployment YAML

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rws
  labels:
    app: rws
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rws
  template:
    metadata:
      labels:
        app: rws
      annotations:
        # Prometheus scrape annotations — tells Prometheus to scrape this pod.
        prometheus.io/scrape: "true"
        prometheus.io/path: "/metrics"
        prometheus.io/port: "7878"
        prometheus.io/scheme: "https"   # the pod serves TLS
    spec:
      terminationGracePeriodSeconds: 30
      containers:
        - name: rws
          image: ghcr.io/bohdaq/rws:18
          ports:
            - name: https
              containerPort: 7878
              protocol: TCP
            - name: quic
              containerPort: 7878
              protocol: UDP
          env:
            - name: RWS_CONFIG_IP
              value: "0.0.0.0"
            - name: RWS_CONFIG_PORT
              value: "7878"
            - name: RWS_CONFIG_THREAD_COUNT
              value: "4"
            - name: RWS_CONFIG_LOG_FORMAT
              value: "json"
            - name: RWS_CONFIG_TLS_CERT_FILE
              value: "/tls/tls.crt"
            - name: RWS_CONFIG_TLS_KEY_FILE
              value: "/tls/tls.key"
          volumeMounts:
            - name: tls
              mountPath: /tls
              readOnly: true
          livenessProbe:
            httpGet:
              path: /healthz
              port: 7878
              scheme: HTTPS   # the port serves TLS; plain-HTTP probes would fail
            initialDelaySeconds: 5
            periodSeconds: 10
            timeoutSeconds: 3
            failureThreshold: 3
          readinessProbe:
            httpGet:
              path: /readyz
              port: 7878
              scheme: HTTPS   # the port serves TLS; plain-HTTP probes would fail
            initialDelaySeconds: 2
            periodSeconds: 5
            timeoutSeconds: 2
            failureThreshold: 2
          resources:
            requests:
              cpu: "100m"
              memory: "64Mi"
            limits:
              cpu: "500m"
              memory: "256Mi"
      volumes:
        - name: tls
          secret:
            secretName: rws-tls
```

Every `RWS_CONFIG_*` variable can come from a ConfigMap or Secret. Settings that name a file, such as `RWS_CONFIG_TLS_CERT_FILE`, take a path, so mount the Secret as a volume (as above) rather than putting its contents in the variable:

```yaml
envFrom:
  - configMapRef:
      name: rws-config   # e.g. RWS_CONFIG_LOG_FORMAT, RWS_CONFIG_THREAD_COUNT
```

## Service YAML

```yaml
apiVersion: v1
kind: Service
metadata:
  name: rws
spec:
  selector:
    app: rws
  ports:
    - name: https
      protocol: TCP
      port: 443
      targetPort: 7878
    - name: quic
      protocol: UDP
      port: 443
      targetPort: 7878
  type: ClusterIP
```

## PodDisruptionBudget

Keep at least two pods running during rolling updates or node drains:

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: rws-pdb
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app: rws
```

## HorizontalPodAutoscaler

Scale on CPU utilisation, or on requests per second per pod. rws exports the
counter `rws_requests_total` at `/metrics`; the
[Prometheus Adapter](https://github.com/kubernetes-sigs/prometheus-adapter)
turns it into the per-pod rate `rws_requests_per_second` in the custom
metrics API. Install it with the rule in
[`helm/prometheus-adapter-values.yaml`](https://github.com/bohdaq/rust-web-server/blob/main/helm/prometheus-adapter-values.yaml):

```bash
helm install prometheus-adapter prometheus-community/prometheus-adapter \
  -n monitoring -f helm/prometheus-adapter-values.yaml   # set prometheus.url first
kubectl get --raw "/apis/custom.metrics.k8s.io/v1beta1/namespaces/default/pods/*/rws_requests_per_second"
```

The rule:

```yaml
rules:
  custom:
    - seriesQuery: 'rws_requests_total{namespace!="",pod!=""}'
      resources:
        overrides:
          namespace: {resource: namespace}
          pod: {resource: pod}
      name:
        matches: "^rws_requests_total$"
        as: "rws_requests_per_second"
      metricsQuery: 'sum(rate(<<.Series>>{<<.LabelMatchers>>}[2m])) by (<<.GroupBy>>)'
```

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rws-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rws
  minReplicas: 2
  maxReplicas: 20
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    # Requests per second per pod, from the Prometheus Adapter rule above.
    - type: Pods
      pods:
        metric:
          name: rws_requests_per_second
        target:
          type: AverageValue
          averageValue: "500"
```

With the Helm chart: `--set autoscaling.enabled=true --set autoscaling.customMetric.enabled=true`.
(Until 18.2.0 this page used an `External` metric named `rws_route_requests_total`, which nothing served.)

## Structured logging for Kubernetes

Access logs are JSON by default (`RWS_CONFIG_LOG_FORMAT=json`, or `log_format = 'json'` in `rws.config.toml`; `combined` gives Combined Log Format, `off`/`none` disables the access log). Kubernetes log aggregators (Fluentd, Fluent Bit, Vector) can then parse and index each field:

```json
{"time":"2026-07-02T12:00:00Z","remote_addr":"10.0.0.1","method":"GET","path":"/api/users","protocol":"HTTP/1.1","status":200,"bytes":1024}
```

:::note[Log to stdout]
`rust-web-server` writes all access logs to stdout (buffered per thread, written at least every 100 ms). Let the container runtime and your log aggregator handle collection and rotation — this is the Kubernetes-native approach.
:::

## Prometheus scraping

With the `prometheus.io/*` annotations on the pod (`prometheus.io/scheme: "https"` when the pod serves TLS), a Prometheus using annotation-based discovery scrapes `GET /metrics` automatically; with the prometheus-operator, enable the chart's `metrics.serviceMonitor` instead. The endpoint returns server-wide counters and — if `MetricsLayer` is enabled — per-route histograms in Prometheus text format. Circuit-breaker state (`rws_circuit_breaker_state{backend}`) is included automatically. The full metric list is on the [Metrics](/features/metrics/) page.

See the [Observability](/deployment/observability) page for example Grafana queries.
