#!/usr/bin/env bash
# .env'deki bilinen secret'ları Vault KV v2'ye yükler (roadmap: "Vault'a
# gerçek API key'lerin populate edilmesi" ops adımının aracı).
#
# Kullanım:
#   VAULT_ADDR=http://127.0.0.1:8200 VAULT_TOKEN=dev-root-token \
#     bash scripts/vault-seed.sh [.env yolu]
#
# Backend'in okuduğu yol ile birebir aynı şema: KV v2, mount `secret`
# (VAULT_MOUNT ile değiştirilebilir), her ad kendi path'inde `value` alanı.
# Boş değerler atlanır. NOT: MVP'de bu adım zorunlu değil — key'ler admin
# panelden de girilebilir; bu script Vault'u tercih eden kurulumlar içindir.

set -euo pipefail

ENV_FILE="${1:-.env}"
MOUNT="${VAULT_MOUNT:-secret}"

if [[ -z "${VAULT_ADDR:-}" || -z "${VAULT_TOKEN:-}" ]]; then
    echo "VAULT_ADDR ve VAULT_TOKEN zorunlu" >&2
    exit 1
fi
if [[ ! -f "$ENV_FILE" ]]; then
    echo "env dosyası yok: $ENV_FILE" >&2
    exit 1
fi

# backend/src/secrets/mod.rs names modülüyle senkron tutulur.
NAMES=(
    JWT_SECRET
    DEEPGRAM_KEY
    ASSEMBLYAI_KEY
    DEEPL_KEY
    GOOGLE_TRANSLATE_KEY
    ELEVENLABS_KEY
    STREAMLABS_CLIENT_ID
    STREAMLABS_CLIENT_SECRET
    YOUTUBE_CLIENT_ID
    YOUTUBE_CLIENT_SECRET
    STRIPE_SECRET_KEY
    STRIPE_WEBHOOK_SECRET
)

seeded=0
for name in "${NAMES[@]}"; do
    # .env'den son tanımı al; yorum/boş satırları ele.
    value="$(grep -E "^${name}=" "$ENV_FILE" | tail -n1 | cut -d= -f2- || true)"
    if [[ -z "$value" ]]; then
        echo "atlandı (boş): $name"
        continue
    fi
    # KV v2 write: POST {addr}/v1/{mount}/data/{name} {"data":{"value":...}}
    payload="$(printf '{"data":{"value":%s}}' "$(printf '%s' "$value" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')")"
    code="$(curl -sS -o /dev/null -w '%{http_code}' \
        -H "X-Vault-Token: ${VAULT_TOKEN}" \
        -H "Content-Type: application/json" \
        -X POST "${VAULT_ADDR%/}/v1/${MOUNT}/data/${name}" \
        -d "$payload")"
    if [[ "$code" != "200" && "$code" != "204" ]]; then
        echo "HATA ($code): $name yazılamadı" >&2
        exit 1
    fi
    echo "yazıldı: $name"
    seeded=$((seeded + 1))
done

echo "bitti: ${seeded} secret Vault'a yüklendi (mount: ${MOUNT})"
