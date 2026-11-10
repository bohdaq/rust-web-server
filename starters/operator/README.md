# {{project-name}}

A Kubernetes operator on [rust-web-server](https://rws8.tech): a custom
resource (`Greeting`), a controller that reconciles it, validating and
mutating webhooks, and the manifests to install it, printed by the binary.

```sh
cargo test                                         # no cluster needed
RWS_TEST_KUBECONFIG=~/.kube/kind cargo test        # also against a throwaway kind/k3d cluster

docker build -t ghcr.io/you/{{project-name}}:0.1.0 .
docker run --rm ghcr.io/you/{{project-name}}:0.1.0 --manifests --image ghcr.io/you/{{project-name}}:0.1.0 --namespace greeter \
  | kubectl apply -f -                             # create the namespace first

kubectl -n greeter apply -f - <<'YAML'
apiVersion: example.com/v1
kind: Greeting
metadata: { name: hello }
spec: { message: "hi" }
YAML
kubectl -n greeter get greetings,configmaps
```

`--manifests` prints the CRD, the RBAC the controllers need, the webhook
configurations, the Deployment (two replicas, one leading) and its Service.
Change `example.com` to your own domain before your first release.
