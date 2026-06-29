# Multi-stage CUDA build of the jesus-twin service.
#
# Build from the REPO ROOT (it needs the jesus-twin/ workspace AND build/rag_corpus.jsonl):
#   docker build -t jesus-twin:local -f deploy/Dockerfile .
#
# Models are NOT baked in — they are multi-GB and git-ignored. They are mounted at runtime
# (see deploy/k3s/30-jesus-twin.yaml: /models/base, /models/embeddinggemma).
#
# ASSUMPTIONS (adjust for your hardware):
#   - CUDA 12.4 (matches the verified box). Change the base image tags if your driver differs.
#   - CUDA_COMPUTE_CAP=89 = NVIDIA L4 / Ada (sm_89). Set to YOUR GPU's compute capability
#     (e.g. 86 for A10/3090, 90 for H100) or candle's CUDA kernels won't match the device.
#   - No frontend yet (no build.rs needs Node). Add a Node stage if/when the Vite UI lands.
#   - If you hit a missing libcudnn at runtime, switch the base images to the `-cudnn-*` variants.

FROM nvidia/cuda:12.4.1-devel-ubuntu22.04 AS builder
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        curl ca-certificates build-essential pkg-config libssl-dev git \
    && rm -rf /var/lib/apt/lists/*
# Rust toolchain. The workspace declares rust-version = 1.85, but its transitive deps need newer
# (roaring -> 1.90, mistralrs/surrealkv/time -> 1.86-1.88). Pin to the host's verified toolchain
# (1.96.0 stable) for parity with the proven local build; bump in lockstep with the host.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.96.0
ENV PATH=/root/.cargo/bin:/usr/local/cuda/bin:$PATH \
    CUDA_COMPUTE_CAP=89
WORKDIR /src
COPY . .
WORKDIR /src/jesus-twin
# First build compiles candle's CUDA kernels (~25 min); the `cuda` feature implies `mistralrs`.
RUN cargo build --release --features cuda -p jesus-twin-cli

# ---- runtime ----
FROM nvidia/cuda:12.4.1-runtime-ubuntu22.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/jesus-twin/target/release/jesus-twin /usr/local/bin/jesus-twin
# The red-letter corpus (small, committed) is baked so the ingest initContainer can read it.
COPY --from=builder /src/build/rag_corpus.jsonl /app/build/rag_corpus.jsonl
# Let the NVIDIA runtime expose the GPU + the libs candle needs.
ENV NVIDIA_VISIBLE_DEVICES=all \
    NVIDIA_DRIVER_CAPABILITIES=compute,utility \
    JESUS_TWIN_ADDR=0.0.0.0:8080
EXPOSE 8080
ENTRYPOINT ["jesus-twin"]
CMD ["serve"]
