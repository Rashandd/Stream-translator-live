# Self-hosted GPU STT servisi (roadmap 2.6, adım 4) — multi-stage build.
# builder (pip + derleme araçları) -> runtime (aynı CUDA taban, derleme
# araçları/pip cache yok). NVIDIA CUDA runtime tabanı zorunlu: faster-whisper
# (CTranslate2) GPU inference için cuDNN/cuBLAS ister.

FROM nvidia/cuda:12.4.1-cudnn-runtime-ubuntu22.04 AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
    python3.11 python3.11-venv python3-pip \
    && rm -rf /var/lib/apt/lists/*

RUN python3.11 -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt

# --- Runtime ---
FROM nvidia/cuda:12.4.1-cudnn-runtime-ubuntu22.04 AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    python3.11 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /opt/venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

COPY main.py ./

EXPOSE 8443

# curl yok (küçük image) — /health'i Python'un kendi stdlib'iyle sorgular.
HEALTHCHECK --interval=30s --timeout=5s --retries=3 --start-period=30s \
    CMD python3.11 -c "import urllib.request,sys; sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:8443/health', timeout=3).status == 200 else 1)"

CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8443"]
