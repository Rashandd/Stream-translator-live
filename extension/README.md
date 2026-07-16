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
npm test          # birim + archive-level package testleri (bağımlılık yok)

# Twitch Hosted Test / staging review ZIP'i:
npm run package:staging

# Production submission ZIP'i:
npm run package:production
```

Komutlar sırasıyla şu artifact'leri üretir (gitignore'ludur):

- `dist/staging/streamdubber-twitch-extension-staging.zip`
- `dist/production/streamdubber-twitch-extension-production.zip`

Sistem `zip` binary'sine bağlı değildir. Paketleyici yalnız Twitch'te çalışan altı
asset'i alır (`viewer.html`, `config.html`, `css/extension.css`, üç `js/` modülü),
sabit ZIP metadata'sı kullanır ve profile özgü URL allowlist'ini archive yazılmadan
önce doğrular. Testler hem ZIP içeriğini hem iki aynı staging build'in byte-for-byte
eşitliğini kontrol eder.

## Ortamlar

Paketleme sırasında seçilebilen profil kümesi kapalıdır; environment variable, URL
flag'i veya query-string override yoktur. Her submitted ZIP kendi ağ hedefini içerir:

| Profil | URL Fetching / WSS EBS origin | Broadcaster dashboard linki |
| --- | --- | --- |
| `staging` | `https://ext-staging.strovak.com` / `wss://ext-staging.strovak.com` | `https://dev.strovak.com/dashboard` |
| `production` | `https://ext.strovak.com` / `wss://ext.strovak.com` | `https://strovak.com/dashboard` |

Staging Hosted Test'e **yalnız** `npm run package:staging` ile üretilen ZIP'i
yükle. Twitch console URL allowlist'inde staging EBS için aynı `https` ve Hosted
Test CSP tarafından doğrulandıktan sonra `wss` originlerini kullan; broadcaster
config'in off-site dashboard URL'i de `https://dev.strovak.com` olmalıdır. Archive
içindeki tek diğer uzak URL Twitch Extension Helper'dır.

`js/shared.js` kaynak dosyası production değerleriyle tutulur; bu değerler sadece
paketleyicinin şablon doğrulaması içindir. Yüklenebilir artifact her zaman yukarıdaki
sabit profillerden biriyle üretilir. `resolveEbsBase()` browser hostname, query
string veya başka bir runtime input okumaz; bu nedenle `?ebs=…` benzeri bir değer
submitted extension'ı başka bir backend'e yönlendiremez.

Backend tarafı: `backend/src/ext_twitch/` (EBS), `ext.twitch.*` config
anahtarları (varsayılan **kapalı**), `TWITCH_EXT_SECRET` yönetilen secret.

## Hosted Test öncesi kontrol

`docs/twitch-extension-policy.md` içindeki Policy Gate Checklist ve Testing
listesi bu paket için bağlayıcıdır (wss CSP doğrulaması, mobile viewport,
slow network, SDK kopması, caption burst, uzun metin, dark/light).
