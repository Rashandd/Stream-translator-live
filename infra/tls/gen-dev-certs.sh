#!/usr/bin/env bash
# Local dev için TLS 1.3 + mTLS sertifikaları üretir.
# ÜRETİMDE KULLANMA — self-signed, sadece geliştirme içindir.
#
# Üretilenler (infra/tls/ altına):
#   ca.crt / ca.key            → kök CA (hem sunucu hem SDK istemci sertifikası imzalar)
#   server.crt / server.key    → backend sunucu sertifikası (TLS_CERT_PATH / TLS_KEY_PATH)
#   client.crt / client.key    → SDK istemci sertifikası (mTLS testi için)
#
# .env içinde:
#   TLS_CERT_PATH=infra/tls/server.crt
#   TLS_KEY_PATH=infra/tls/server.key
#   TLS_CLIENT_CA_PATH=infra/tls/ca.crt   # mTLS için

set -euo pipefail
cd "$(dirname "$0")"

DAYS=825
SUBJ_CA="/CN=StreamDubber Dev CA"
SUBJ_SERVER="/CN=localhost"
SUBJ_CLIENT="/CN=streamdubber-sdk"

echo "→ Kök CA üretiliyor..."
openssl genrsa -out ca.key 4096
openssl req -x509 -new -nodes -key ca.key -sha256 -days "$DAYS" \
  -subj "$SUBJ_CA" -out ca.crt

echo "→ Sunucu sertifikası (localhost) üretiliyor..."
openssl genrsa -out server.key 2048
openssl req -new -key server.key -subj "$SUBJ_SERVER" -out server.csr
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -days "$DAYS" -sha256 \
  -extfile <(printf "subjectAltName=DNS:localhost,IP:127.0.0.1") \
  -out server.crt

echo "→ SDK istemci sertifikası (mTLS) üretiliyor..."
openssl genrsa -out client.key 2048
openssl req -new -key client.key -subj "$SUBJ_CLIENT" -out client.csr
openssl x509 -req -in client.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -days "$DAYS" -sha256 -out client.crt

rm -f server.csr client.csr ca.srl
echo "✓ Sertifikalar infra/tls/ altında hazır."
