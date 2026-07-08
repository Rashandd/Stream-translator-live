"""`stt-server`'ın /transcribe ve /health sözleşme testleri.

`FasterWhisperEngine` gerçek CUDA/model indirme gerektirdiği için burada hiç
kullanılmaz — `main.engine`'i bir `FakeEngine`'le değiştirip HTTP katmanını
(auth, boş ses, yanıt şekli) GPU'suz test eder.
"""

import os
import struct
import sys
from pathlib import Path

os.environ.setdefault("API_TOKEN", "test-token")

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import numpy as np
from fastapi.testclient import TestClient

import main as stt_main


class FakeEngine:
    def __init__(self, text: str = "merhaba dünya"):
        self.text = text
        self.calls: list[tuple[int, str]] = []

    def transcribe(self, pcm, language: str) -> str:
        self.calls.append((len(pcm), language))
        return self.text


def pcm_bytes(values=None, n_samples: int = 1600) -> bytes:
    if values is None:
        values = [1000] * n_samples
    return struct.pack(f"<{len(values)}h", *values)


def make_client(engine: FakeEngine, token: str = "test-token") -> TestClient:
    stt_main.engine = engine
    stt_main.API_TOKEN = token
    return TestClient(stt_main.app)


def test_transcribe_requires_authorization_header():
    client = make_client(FakeEngine())
    resp = client.post("/transcribe?language=tr", content=pcm_bytes())
    assert resp.status_code == 401


def test_transcribe_rejects_wrong_token():
    client = make_client(FakeEngine())
    resp = client.post(
        "/transcribe?language=tr",
        content=pcm_bytes(),
        headers={"Authorization": "Bearer wrong-token"},
    )
    assert resp.status_code == 401


def test_transcribe_returns_expected_shape():
    engine = FakeEngine("merhaba")
    client = make_client(engine)
    resp = client.post(
        "/transcribe?language=tr",
        content=pcm_bytes(),
        headers={"Authorization": "Bearer test-token"},
    )
    assert resp.status_code == 200
    assert resp.json() == {"text": "merhaba", "is_final": True}
    assert engine.calls == [(1600, "tr")]


def test_empty_audio_is_rejected():
    client = make_client(FakeEngine())
    resp = client.post(
        "/transcribe?language=tr",
        content=b"",
        headers={"Authorization": "Bearer test-token"},
    )
    assert resp.status_code == 400


def test_missing_api_token_config_is_server_error():
    client = make_client(FakeEngine(), token="")
    resp = client.post(
        "/transcribe?language=tr",
        content=pcm_bytes(),
        headers={"Authorization": "Bearer anything"},
    )
    assert resp.status_code == 500


def test_health_reports_ok_without_gpu():
    client = make_client(FakeEngine())
    resp = client.get("/health")
    assert resp.status_code == 200
    body = resp.json()
    assert body["status"] == "ok"
    assert body["model"] == stt_main.MODEL_NAME
    # Bu makinede/CI'da NVIDIA GPU yoksa None olmalı, hata değil.
    assert "gpu_name" in body and "vram_mb" in body


def test_pcm_conversion_normalizes_int16_to_float32():
    raw = pcm_bytes(values=[0, 16384, -32768, 32767])
    arr = stt_main.pcm_bytes_to_float32(raw)
    assert arr.dtype == np.float32
    assert arr.shape == (4,)
    assert abs(arr[0] - 0.0) < 1e-6
    assert abs(arr[1] - 0.5) < 1e-3
    assert abs(arr[2] - (-1.0)) < 1e-3
