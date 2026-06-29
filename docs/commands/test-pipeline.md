---
name: test-pipeline
description: STT + çeviri pipeline'ını uçtan uca test et
disable-model-invocation: true
---

Ses pipeline'ını uçtan uca test et:

1. Backend WebSocket sunucusu çalışıyor mu: `cargo run --bin backend`
2. Test ses dosyası gönder: `cargo test integration::stt_pipeline`
3. Deepgram yanıt süresi <300ms mı kontrol et
4. DeepL çeviri <100ms mı kontrol et
5. Toplam latency <2sn mi kontrol et
6. VAD boş ses engeli çalışıyor mu: sessiz ses gönder, buluta gitmemeli
7. AES şifreleme aktif mi: Wireshark veya log kontrol
8. Sonuçları yaz: geçti / kaldı, nerede tıkandı
