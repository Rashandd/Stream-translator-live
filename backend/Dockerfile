# StreamDubber backend — multi-stage build.
# builder (cargo) → distroless runtime (küçük, shell yok).

# Cargo.lock'taki seçili bağımlılıkların MSRV'si 1.88.
FROM rust:1.88-bookworm@sha256:af306cfa71d987911a781c37b59d7d67d934f49684058f96cf72079c3626bfe0 AS builder
WORKDIR /build

# aws-lc-rs/ring derlemesi için cmake + clang gerekir.
RUN apt-get update && apt-get install -y --no-install-recommends cmake clang \
    && rm -rf /var/lib/apt/lists/*

# Workspace manifesti + backend crate.
COPY Cargo.toml Cargo.lock ./
COPY sdk/src-tauri/Cargo.toml sdk/src-tauri/Cargo.toml
COPY backend/ backend/

RUN cargo build --release --locked --bin streamdubber-backend

# --- Runtime ---
FROM gcr.io/distroless/cc-debian12@sha256:7ee09f36862efbdbf70422db263e411c2618409ca46faa555bd5b636155307df AS runtime
WORKDIR /app

COPY --from=builder /build/target/release/streamdubber-backend /usr/local/bin/streamdubber-backend
# Migration'lar binary'ye gömülü (sqlx::migrate!), ayrıca kopyalamaya gerek yok.

# Self-hosted GPU STT provisioning (roadmap 2.6): SshProvisioner bootstrap.sh'i
# + main.py/requirements.txt'i (Docker kullanılamayan hedeflerde — ör. Vast.ai
# container instance — bare-mode kurulum için) STT_BOOTSTRAP_SCRIPT/STT_APP_DIR
# yollarından okur — image'a gömülü olmazlarsa provisioning açık bir hatayla
# başarısız olur, backend çökmez.
COPY infra/stt-server/bootstrap.sh /app/infra/stt-server/bootstrap.sh
COPY stt-server/main.py stt-server/requirements.txt /app/stt-server/
ENV STT_BOOTSTRAP_SCRIPT=/app/infra/stt-server/bootstrap.sh
ENV STT_APP_DIR=/app/stt-server

EXPOSE 8080

# Healthcheck binary'nin kendi --health-check moduyla (distroless'ta curl yok).
HEALTHCHECK --interval=30s --timeout=5s --retries=3 --start-period=10s \
    CMD ["/usr/local/bin/streamdubber-backend", "--health-check"]

ENTRYPOINT ["/usr/local/bin/streamdubber-backend"]
