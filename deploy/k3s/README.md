# Deploying jesus-twin to local k3s (GPU + Traefik Gateway API + Let's Encrypt)

This deploys the RAG-first jesus-twin service to a **single-node, GPU-enabled k3s** cluster,
exposed at **`https://jesus-twin.know-me.tools`** with an automatic Let's Encrypt certificate.

We use **k3s's bundled Traefik** as the Gateway API controller — Traefik v3 is a fully
conformant Gateway API implementation, so there is **no need for a second controller (Envoy
Gateway)**. The gateway choice is the Gateway *API* (the open standard); Traefik is just the
implementation already running. Edge rate-limiting/auth — the usual reason to reach for Envoy —
is already owned in-app by `jesus-twin-admission` (the parking-lot gatekeeper), so Traefik is
sufficient here.

## Two gotchas that will bite (read first)

1. **We use ACME DNS-01 via Cloudflare** (see `10-cert-manager-clusterissuer.yaml`). cert-manager
   writes a TXT record through the Cloudflare API to prove ownership, so: the A record can stay
   **proxied (orange cloud)** — origin IP hidden, Cloudflare in front; **no inbound port 80** is
   needed for issuance; wildcard certs are possible. You must create the Cloudflare token Secret
   (`15-cloudflare-secret.example.yaml`). The A record for `jesus-twin.know-me.tools` still has to
   exist and point at your public IP (proxied is fine) so real traffic reaches the gateway on 443.
2. **k3s Traefik listens on `8000`/`8443` internally**, exposed as `80`/`443` by its
   LoadBalancer Service. Gateway listener **ports must be `8000`/`8443`** (see `20-gateway.yaml`),
   not 80/443, or the listener is rejected (`ListenersNotValid`).

## Prerequisites on the node

- NVIDIA driver + **NVIDIA Container Toolkit** installed, and the toolkit registered with k3s'
  containerd: `sudo nvidia-ctk runtime configure --runtime=containerd \
  --config=/var/lib/rancher/k3s/agent/etc/containerd/config.toml && sudo systemctl restart k3s`.
- The model directories already on the node (they are multi-GB and git-ignored — we **mount**
  them, never bake them into the image):
  - base Gemma 4 checkpoint  → `/usr/local/src/jesus-twin/jesus-twin-base`
  - embeddinggemma           → `/usr/local/src/jesus-twin/jesus-twin-embeddinggemma`
  We ship **RAG-first on the base model** (the merged fine-tune is degenerate — see
  `docs/FINDINGS.md`). Repoint `JESUS_TWIN_MODEL` later if a good checkpoint lands.
- `kubectl` + `helm` pointed at the cluster (`export KUBECONFIG=/etc/rancher/k3s/k3s.yaml`).
- The container image built and loadable by the node — see `../Dockerfile`. For a single node:
  `docker build -t jesus-twin:local -f deploy/Dockerfile . && \
   docker save jesus-twin:local | sudo k3s ctr images import -`.

## Apply order

```bash
# 1. Gateway API CRDs (required by BOTH Traefik's gateway provider AND cert-manager; install first)
kubectl apply -f https://github.com/kubernetes-sigs/gateway-api/releases/download/v1.2.1/standard-install.yaml

# 2. Turn on Traefik's Gateway API provider (k3s reads this HelmChartConfig automatically)
sudo cp deploy/k3s/00-traefik-gateway-helmchartconfig.yaml \
        /var/lib/rancher/k3s/server/manifests/   # k3s applies it; or `kubectl apply -f` it

# 3. cert-manager, WITH Gateway API support enabled (the gateway-shim that auto-creates the
#    Certificate from the annotated Gateway watches Gateway resources, so this is needed even
#    though the DNS-01 challenge itself doesn't use the Gateway).
helm repo add jetstack https://charts.jetstack.io && helm repo update
helm install cert-manager jetstack/cert-manager \
  --namespace cert-manager --create-namespace \
  --set crds.enabled=true \
  --set "extraArgs={--feature-gates=ExperimentalGatewayAPISupport=true}"
# (cert-manager >= ~1.18 uses `--set config.enableGatewayAPI=true` instead of the feature gate.)

# 3b. The Cloudflare API token Secret for the DNS-01 solver (NOT committed; create imperatively):
kubectl create secret generic cloudflare-api-token \
  -n cert-manager --from-literal=api-token=<YOUR_CLOUDFLARE_API_TOKEN>

# 4. The rest
kubectl apply -f deploy/k3s/nvidia-runtimeclass.yaml
kubectl apply -f deploy/k3s/10-cert-manager-clusterissuer.yaml
kubectl apply -f deploy/k3s/20-gateway.yaml
kubectl apply -f deploy/k3s/30-jesus-twin.yaml   # initContainer ingests the red-letter corpus once
```

The red-letter corpus is loaded into the persistent store by the Deployment's **initContainer**
(idempotent via a `.ingested` marker) — the embedded SurrealDB is single-process, so this must
happen before the server opens the store, not as a concurrent Job. To also load the Tanakh /
Gospel-narrative / principle-tag corpora (which exercise the source/narrative blocks), extend that
initContainer with the matching `jesus-twin ingest-tanakh` / `ingest-gospel-narrative` /
`apply-principle-tags` calls once their `build/*.jsonl` are baked into the image.

## Verify

```bash
kubectl -n jesus-twin get gateway,httproute,certificate,pods
kubectl -n jesus-twin describe certificate jesus-twin-tls   # Ready=True once HTTP-01 passes
# GPU is actually scheduled:
kubectl -n jesus-twin exec deploy/jesus-twin -- nvidia-smi
curl https://jesus-twin.know-me.tools/v1/models                 # OpenAI surface, valid TLS
```

If the certificate stays `Ready=False`: check `kubectl -n jesus-twin describe challenge` — the
usual cause is the A record being **proxied** (gotcha #1) or 80/443 not forwarded to this node.
