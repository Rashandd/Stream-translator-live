---
segment: platform referansı (01–04 tüm dosyalardan link alır)
dashboard_routes:
  - /dashboard/platforms
  - /dashboard/distribution
api_endpoints:
  - GET /api/integrations, /api/integrations/{provider}/authorize|test, DELETE /api/integrations/{provider}
  - POST /api/ext/twitch/code, GET/DELETE /api/ext/twitch/binding
  - GET /ext/twitch/captions/{channel_id} (viewer WS)
---

# Platform Notları — Twitch / YouTube / Kick

Segment dokümanları bu dosyaya link verir; platforma özgü tüm detaylar
burada toplanır.

## Özet Matris

| | Twitch | YouTube | Kick |
|---|---|---|---|
| Overlay altyazı (Browser/Web Source) | `[HAZIR]` | `[HAZIR]` | `[HAZIR]` |
| İzleyicinin kendi dilini seçmesi | `[YAKINDA]` — Twitch Extension | `[YAKINDA]` — Native CC | `[PLANLANDI]` — Chrome Extension |
| Dublaj için ikinci hesap | `[HAZIR]` (device-mux) | `[HAZIR]` (device-mux) | `[HAZIR]` (device-mux) |
| Yayıncı girişi/hesap bağlama | `[HAZIR]` | `[HAZIR]` | Bağlama: `[HAZIR]` · Giriş: `[YAKINDA]` |
| Tek hesap yeterli mi? | Hayır — dublaj için 2. hesap gerekir | Faz 3/4'te multi-audio ile evet olacak, bugün hayır | Hayır — dublaj için 2. hesap gerekir |

---

## Twitch

- **Hesap bağlama/giriş:** `[HAZIR]`. Twitch, hem sosyal giriş (`/login`,
  `/signup` sosyal butonlar) hem de içerik-platformu bağlama
  (`/dashboard/platforms`) için ayrı OAuth client'larla kullanılır — ikisi
  bağımsız çalışır.
- **İzleyici dil seçimi (Twitch Extension):** `[YAKINDA]`. Backend (EBS,
  JWT doğrulama, kanal bağlama, canlı caption WS) ve component overlay
  (izleyici bar'ı, dil seçici, aç/kapat) **kodlanmış ve testli**; yayıncı
  tarafı kanal bağlama kartı (`/dashboard/platforms`) bugün canlı çalışıyor.
  Eksik olan: **Twitch'in kendi console kaydı + Hosted Test + review
  başvurusu** — bunlar tamamlanmadan extension normal izleyicilerin
  Twitch'te göreceği bir "gerçek" extension olarak yayınlanamaz (insan/ops
  adımı, 2-4 hafta sürebilir). O zamana kadar izleyici dil seçimi, overlay
  URL'i farklı `?lang=` değerleriyle ayrı Browser Source'lar olarak
  kurmakla (yayıncının kendi seçtiği sabit dil(ler)) sağlanır.
- **Dublaj (ikinci hesap):** `[HAZIR]`. Cihazda mux ile TR ses → hesap 1,
  klonlanmış ses → hesap 2 çalışır (gerçek ffmpeg ile uçtan uca test edildi).
- **Ban riski:** Twitch aynı IP'den bağlanan iki hesabı ban edebilir — SDK
  bağlantı/profil ayrımı öneriyor (bkz. `03-dublaj-ses-klonlama.md` ön koşullar).

## YouTube

- **Hesap bağlama:** `[HAZIR]` — `/dashboard/platforms`, `youtube.force-ssl`
  scope'uyla, offline+consent (uzun ömürlü refresh token için).
- **Giriş (login):** `[HAZIR]` — Google ile sosyal giriş, platform
  bağlamadan bağımsız ayrı bir OAuth client kullanır (aynı Google hesabı
  her ikisine de izin verebilir).
- **İzleyici dil seçimi (native CC):** `[YAKINDA]`. `closedCaptionsIngestionUrl`'e
  UTF-8 metin push eden istemci kodlanmış (`backend/src/integrations/youtube.rs`),
  ama **canlı yayın döngüsüne henüz bağlanmadı** — bugün gerçek bir YouTube
  yayınına CC göndermiyor. O zamana kadar YouTube yayıncısı da diğer
  platformlar gibi overlay/Browser Source yöntemini kullanır.
- **Multi-audio (ayrı ses parçası başına dil):** `[PLANLANDI]` — Faz 3/4.
- **Dublaj (ikinci kanal):** `[HAZIR]` — mekanizma platform-agnostik (device-mux).
  Aynı-IP ban riski YouTube için ayrıca belgelenmedi; temkinli ol.

## Kick

- **Giriş (login):** `[YAKINDA]` — Kick OAuth 2.1 + PKCE gerektiriyor,
  bugünkü OAuth çekirdeği (`exchange_code`) yalnız `client_secret` akışını
  destekliyor; ayrı bir işe ertelendi.
- **Hesap bağlama (overlay/relay hedefi için):** `[HAZIR]` — Kick'e RTMP
  hedefi olarak bağlanmak (relay/dublaj hedefi) OAuth login'e ihtiyaç
  duymaz, yalnız RTMP URL + stream key gerekir (`PUT /api/relay/targets`).
- **İzleyici dil seçimi (Chrome Extension):** `[PLANLANDI]` — roadmap 2.5,
  hiç başlanmadı (Manifest V3 + Chrome Web Store başvurusu ikisi de boş).
- **Dublaj (ikinci hesap):** `[HAZIR]` — device-mux, platform-agnostik.
- **Ban riski:** Kick de Twitch gibi aynı IP'den iki hesabı ban edebilir.

---

## Notlar

- Üç platformda da **overlay/Browser Source yöntemi bugün aynı şekilde
  çalışır** — yayın yazılımı (OBS/Streamlabs Desktop/XSplit/vMix/capture-card
  arkasından PC) Strovak için farksızdır, çünkü overlay tarayıcı tabanlı bir
  Web Source'tur.
- İzleyicinin **kendi dilini seçebilmesi** (platforma native bir mekanizma
  ile) üç platformda da farklı aşamada: Twitch en yakın (yalnız insan/ops
  adımı kaldı), YouTube kodlanmış ama bağlanmamış, Kick hiç başlanmadı.
- Detaylı arka plan için: `docs/multistream.md`, `docs/twitch-extension-plan.md`,
  `docs/twitch-extension-production-readiness-plan.md`.
