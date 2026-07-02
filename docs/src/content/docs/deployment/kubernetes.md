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
        # Prometheus scrape annotation — tells Prometheus to scrape this pod.
        prometheus.io/scrape: "true"
        prometheus.io/path: "/metrics"
        prometheus.io/port: "7878"
    spec:
      terminationGracePeriodSeconds: 30
      containers:
        - name: rws
          image: ghcr.io/your-org/rws:latest
          ports:
            - name: http
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
            initialDelaySeconds: 5
            periodSeconds: 10
            timeoutSeconds: 3
            failureThreshold: 3
          readinessProbe:
            httpGet:
              path: /readyz
              port: 7878
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

Scale on CPU utilisation, or on custom Prometheus metrics via the KEDA adapter:

```yaml
# CPU-based autoscaling (built-in metrics server)
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
    # Custom metric: request rate from Prometheus (requires KEDA or the
    # Prometheus adapter). Metric name matches what MetricsLayer exposes.
    - type: External
      external:
        metric:
          name: rws_route_requests_total
        target:
          type: AverageValue
          averageValue: "1000"
```

## Structured logging for Kubernetes

Set `RWS_CONFIG_LOG_FORMAT=json` to emit access logs in JSON format. Kubernetes log aggregators (Fluentd, Fluent Bit, Vector) can then parse and index each field:

```json
{"time":"2026-07-02T12:00:00Z","remote_addr":"10.0.0.1","method":"GET","uri":"/api/users","status":200,"bytes":1024,"duration_ms":3}
```

:::note[Log to stdout]
`rust-web-server` writes all access logs to stdout. Let the container runtime and your log aggregator handle collection and rotation — this is the Kubernetes-native approach.
:::

## Prometheus scraping

With `prometheus.io/scrape: "true"` annotation on the pod, a standard Prometheus installation scrapes `GET /metrics` automatically. The endpoint returns server-wide counters and — if `MetricsLayer` is enabled — per-route histograms in Prometheus text format.

See the [Observability](/deployment/observability) page for example Grafana queries.
