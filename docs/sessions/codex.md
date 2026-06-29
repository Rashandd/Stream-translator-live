# Codex Session

## Agent
Codex

## Neredeyim?
Proje dokumantasyonu analiz edildi ve multi-agent session yapisi kuruluyor.

## Degistirdigim Alanlar
- `docs/SESSION.md`
- `docs/sessions/`
- `AGENTS.md` session/handoff kurallari

## Son Kararlar
- Agentlar calisma sonunda ana `docs/SESSION.md` yerine kendi session dosyasini gunceller.
- `docs/SESSION.md` kisa koordinasyon panosu olarak kullanilir.

## Sonraki Adim
Backend iskeletini olustur: Rust workspace/backend crate, Axum health endpoint, WebSocket route modulu, audio frame parser testleri ve implementasyonu.

## Blokerlar / Dikkat
- Ayni anda Claude calisiyorsa `AGENTS.md` ve `docs/SESSION.md` uzerinde yazma cakismasi kontrol edilmeli.
- `docs/schema.md` ve `docs/websocket-protocol.md` git'e eklenmemis gorunuyor; kullanici/Claude degisikligi gibi ele alinmali.
