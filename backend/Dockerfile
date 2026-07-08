# StreamDubber backend — multi-stage build.
# builder (cargo) → distroless runtime (küçük, shell yok).

FROM rust:1.85-bookworm AS builder
WORKDIR /build

# aws-lc-rs/ring derlemesi için cmake + clang gerekir.
RUN apt-get update && apt-get install -y --no-install-recommends cmake clang \
    && rm -rf /var/lib/apt/lists/*

# Workspace manifesti + backend crate.
COPY Cargo.toml Cargo.lock ./
COPY backend/ backend/

RUN cargo build --release --bin streamdubber-backend

# --- Runtime ---
FROM gcr.io/distroless/cc-debian12 AS runtime
WORKDIR /app

COPY --from=builder /build/target/release/streamdubber-backend /usr/local/bin/streamdubber-backend
# Migration'lar binary'ye gömülü (sqlx::migrate!), ayrıca kopyalamaya gerek yok.

# Self-hosted GPU STT provisioning (roadmap 2.6): SshProvisioner bu script'i
# STT_BOOTSTRAP_SCRIPT yoluyla okur (varsayılan aynı yol) — image'a gömülü
# olmazsa provisioning açık bir hatayla başarısız olur, backend çökmez.
COPY infra/stt-server/bootstrap.sh /app/infra/stt-server/bootstrap.sh
ENV STT_BOOTSTRAP_SCRIPT=/app/infra/stt-server/bootstrap.sh

EXPOSE 8080

# Healthcheck binary'nin kendi --health-check moduyla (distroless'ta curl yok).
HEALTHCHECK --interval=30s --timeout=5s --retries=3 --start-period=10s \
    CMD ["/usr/local/bin/streamdubber-backend", "--health-check"]

ENTRYPOINT ["/usr/local/bin/streamdubber-backend"]
