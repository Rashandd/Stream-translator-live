---
name: test-pipeline
description: STT + çeviri pipeline'ını uçtan uca test et
---

Ses pipeline'ını uçtan uca test et:

1. Backend WebSocket sunucusu çalışıyor mu: `cargo run --bin backend`
2. Test ses dosyası gönder: `cargo test integration::stt_pipeline`
3. Deepgram yanıt süresi <300ms mı kontrol et
4. DeepL çeviri <100ms mı kontrol et
5. Toplam latency <2sn mi kontrol et
6. VAD boş ses engeli çalışıyor mu: sessiz ses gönder, buluta gitmemeli
7. TLS 1.3 aktif mi, ses verisi şifreli gidiyor mu: Wireshark veya log kontrol (AES bekleme — double encryption olmamalı)
8. Sonuçları yaz: geçti / kaldı, nerede tıkandı
