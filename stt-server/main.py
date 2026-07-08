"""Self-hosted GPU STT servisi (roadmap 2.6, adım 4).

faster-whisper (CTranslate2) large-v3-turbo modelini saran minimal FastAPI
servisi — bir GPU'lu VPS/VDS'e (sağlayıcıdan bağımsız) kurulur, backend
Deepgram yerine buraya çağrı yapar (`backend/src/gateway/selfhosted_stt.rs`).

`/transcribe` ham istek gövdesi WS pipeline'ın Deepgram'a da yolladığı aynı
formattır: 16 kHz mono s16le PCM (bkz. `sdk/src/core/local_stt.rs` dokümanı —
"aynı format bulut yoluyla akar"). Yanıt şekli backend'in beklediğiyle
birebir: `{"text": ..., "is_final": true}`.
"""

from __future__ import annotations

import os
import subprocess
from typing import Optional, Protocol

import numpy as np
from fastapi import FastAPI, Header, HTTPException, Request
from fastapi.responses import JSONResponse

MODEL_NAME = os.environ.get("STT_MODEL", "large-v3-turbo")
API_TOKEN = os.environ.get("API_TOKEN", "")


class Engine(Protocol):
    def transcribe(self, pcm: np.ndarray, language: str) -> str: ...


class FasterWhisperEngine:
    """Gerçek motor — model ilk çağrıda tembel yüklenir (import/model indirme
    süreç başlangıcını geciktirmesin, `/health` GPU olmadan da yanıt versin)."""

    def __init__(self, model_name: str = MODEL_NAME):
        self._model_name = model_name
        self._model = None

    def _ensure_loaded(self):
        if self._model is None:
            from faster_whisper import WhisperModel

            self._model = WhisperModel(
                self._model_name, device="cuda", compute_type="int8_float16"
            )
        return self._model

    def transcribe(self, pcm: np.ndarray, language: str) -> str:
        model = self._ensure_loaded()
        segments, _info = model.transcribe(pcm, language=language)
        return "".join(segment.text for segment in segments).strip()


app = FastAPI()
# Modül seviyesi değişken bilinçli olarak mutable — testler `main.engine`'i
# bir `FakeEngine`'le değiştirir (Rust tarafındaki `LocalSttEngine`/`MockEngine`
# desenindeki aynı test-edilebilirlik ilkesi).
engine: Engine = FasterWhisperEngine()


def pcm_bytes_to_float32(raw: bytes) -> np.ndarray:
    """16-bit little-endian PCM -> [-1, 1] float32 (faster-whisper girdisi)."""
    samples = np.frombuffer(raw, dtype="<i2")
    return samples.astype(np.float32) / 32768.0


def _require_token(authorization: Optional[str]) -> None:
    if not API_TOKEN:
        raise HTTPException(status_code=500, detail="API_TOKEN not configured")
    if authorization != f"Bearer {API_TOKEN}":
        raise HTTPException(status_code=401, detail="invalid token")


@app.post("/transcribe")
async def transcribe(
    request: Request,
    language: str = "tr",
    authorization: Optional[str] = Header(default=None),
):
    _require_token(authorization)
    raw = await request.body()
    if not raw:
        raise HTTPException(status_code=400, detail="empty audio")

    pcm = pcm_bytes_to_float32(raw)
    text = engine.transcribe(pcm, language)
    return JSONResponse({"text": text, "is_final": True})


def _probe_gpu() -> tuple[Optional[str], Optional[int]]:
    """`nvidia-smi`'ye shell-out eder — backend'in `gpu_probe.rs`'iyle aynı
    best-effort desen: GPU/binary yoksa None döner, hata fırlatmaz."""
    try:
        out = subprocess.run(
            ["nvidia-smi", "--query-gpu=name,memory.total", "--format=csv,noheader,nounits"],
            capture_output=True,
            text=True,
            timeout=5,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None, None
    if out.returncode != 0 or not out.stdout.strip():
        return None, None
    line = out.stdout.strip().splitlines()[0]
    if "," not in line:
        return None, None
    name, vram = line.rsplit(",", 1)
    name = name.strip()
    try:
        vram_mb = int(vram.strip())
    except ValueError:
        return None, None
    if not name or vram_mb <= 0:
        return None, None
    return name, vram_mb


@app.get("/health")
async def health():
    gpu_name, vram_mb = _probe_gpu()
    return JSONResponse(
        {
            "status": "ok",
            "model": MODEL_NAME,
            "gpu_name": gpu_name,
            "vram_mb": vram_mb,
        }
    )
