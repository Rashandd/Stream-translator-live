# StreamDubber Live Captions — Twitch Extension

Twitch Video Overlay Extension (Faz 2.4, subtitles-only ilk release).
Policy ve review kuralları: `docs/twitch-extension-policy.md` · Uygulama planı: `docs/twitch-extension-plan.md`

## Tasarım kararları

- **Bundler yok.** Kaynak dosyalar (ES modules) review'a giden bundle'ın
  kendisidir — Twitch "human-readable JS" şartını doğrudan karşılar. Kod
  yorumları Twitch reviewer'ları için İngilizcedir.
- **Framework yok, webfont yok.** Toplam paket birkaç KB; mobile 1 MB / 3 sn
  bütçesinin çok altında. Font system-ui stack'idir.
- **`textContent`-only render.** Ağdan gelen hiçbir veri HTML olarak
  yorumlanmaz; dil kodları allowlist regex'inden geçer (`js/shared.js`).
- **Transport gate.** Önce WebSocket (`/ext/twitch/captions/{channel}`);
  art arda başarısızlıkta state endpoint'ine (`/ext/twitch/state/{channel}`)
  kısa aralıklı poll'a düşer, soketi arada yeniden dener (policy fallback şartı).
- **Kimlik istenmez.** Dil/boyut/görünürlük tercihleri anonim `localStorage`'ta.
- **Emoji yok; ikonlar inline lucide SVG.**

## Dosyalar

| Dosya | İş |
|---|---|
| `viewer.html` + `js/viewer.js` | İzleyici overlay'i: dil seçimi, aç/kapat, boyut, durum göstergesi |
| `config.html` + `js/config.js` | Broadcaster config: 6 haneli kodla kanal↔hesap bağlama, varsayılan dil |
| `js/shared.js` | Saf, test edilebilir mantık (parse, dil, durum makinesi, backoff, tercihler) |
| `css/extension.css` | DESIGN.md paletine bağlı stiller (navy, pill, status renkleri) |
| `tests/` | `node --test` birim testleri |

## Komutlar

```bash
npm test          # birim testleri (node --test, bağımlılık yok)

# Review zip'i — SADECE çalışan dosyalar girer (extraneous dosya yasak):
zip -r streamdubber-twitch-extension.zip viewer.html config.html css js
```

## Ortamlar

`js/shared.js` içindeki `EBS_BASE_PRODUCTION` (`https://ext.strovak.com`) her
release'te Twitch URL allowlist'iyle birebir aynı olmalıdır. Local geliştirme /
Developer Rig için sayfa `localhost`'tan açıldığında `?ebs=http://localhost:8080`
override'ı çalışır; hosted ortamda override yok sayılır.

Backend tarafı: `backend/src/ext_twitch/` (EBS), `ext.twitch.*` config
anahtarları (varsayılan **kapalı**), `TWITCH_EXT_SECRET` yönetilen secret.

## Hosted Test öncesi kontrol

`docs/twitch-extension-policy.md` içindeki Policy Gate Checklist ve Testing
listesi bu paket için bağlayıcıdır (wss CSP doğrulaması, mobile viewport,
slow network, SDK kopması, caption burst, uzun metin, dark/light).
