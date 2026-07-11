# Production Operator Guide

Amaç: production'da teknik işlem gerektirmeden sistemi yönetmek. Deploy bir kez
hazırlanır; sonrasında sağlayıcı key'leri, OAuth client bilgileri, Stripe fiyat
ID'leri, relay hedefleri ve testler admin panelden yürütülür.

## Sorumluluk Ayrımı

### Kullanıcı / Operator

Kod yazmaz, sunucuya SSH ile girmez, secret dosyası düzenlemez.

- Admin paneline girer.
- Provider hesaplarını açar: Google, Stripe, Deepgram, DeepL, Streamlabs.
- Admin > Secrets ekranına key/client bilgilerini girer.
- Her sağlayıcı için Test butonuna basar.
- Stripe price ID'lerini admin config'e girer.
- Yayıncı hesaplarını ve RTMP hedeflerini web panelden yönetir.
- Canlı smoke checklist'ini takip eder: OBS, YouTube/Twitch/Kick, latency,
  Stripe checkout, Google OAuth.

### AI Agent / Developer

Operator'ın admin panelden yapamadığı her şeyi ürüne ekler.

- Hiçbir provider key'ini veya fiyat bilgisini koda gömmez.
- Her production ayarını admin panel, DB/config veya managed secret katmanından
  okunur hale getirir.
- Eksik UI wiring'i tamamlar: setup wizard, provider status, OAuth connect,
  relay target sync, price ID edit.
- Test ve smoke scriptlerini tutar.
- Roadmap tiklerini ancak test/smoke geçince kapatır.
- Provider dokümantasyonunu canlı console ekranları değiştikçe günceller.

## Production Profile Yapısı

Production'da üç ayrı profil düşünülür:

| Profil | Nerede tutulur | Kim yönetir | Amaç |
| --- | --- | --- | --- |
| Deploy profile | VPS env (`infra/deploy.md`) | Operator ilk deploy'da girer | Uygulamanın açılması |
| Provider profile | Admin > Secrets + Admin Config | Operator | Google, Stripe, Deepgram, DeepL, Streamlabs |
| Broadcaster profile | Yayıncı paneli | Yayıncı/operator | RTMP hedefleri, stream key, overlay, dil seçimi |

### Deploy Profile

Minimum production env:

```env
BIND_ADDR=0.0.0.0:8080
JWT_SECRET=<32+ byte random>
DATABASE_URL=<postgres url>
REDIS_URL=<redis url>
ADMIN_EMAIL=<admin email>
ADMIN_PASSWORD_HASH=<argon2id hash>
SECRETS_FILE=/data/secrets.enc
TOKENS_FILE=/data/tokens.enc
RELAY_FILE=/data/relay.enc
USAGE_FILE=/data/usage.jsonl
LOG_FILE=/data/logs.jsonl
SECRETS_ENC_KEY=<32 byte key, recommended>
```

Provider key'leri env'e girmek zorunlu değildir. Production hedefi: key'ler
admin panelden girilir ve şifreli kalıcı dosyada saklanır.

### Provider Profile

Her provider için tek düzen:

| Alan | Örnek |
| --- | --- |
| Provider | `youtube`, `google_translate`, `stripe`, `deepl`, `deepgram` |
| Environment | `production` |
| Secret names | `YOUTUBE_CLIENT_ID`, `YOUTUBE_CLIENT_SECRET` |
| Callback URL | `https://api.strovak.com/api/integrations/youtube/callback` |
| Status | `unset`, `configured`, `tested`, `connected`, `failed` |
| Last tested at | Admin panel Provider Test zamanı |
| Owner | Hangi hesap/proje ile açıldı |

AI agent'ların hedefi bu profil modelini panelde anlaşılır hale getirmek:
operator sadece değer girer, Test/Connect butonuna basar, sonuç görür.

### Broadcaster Profile

RTMP relay için yayıncı başına saklanacak yapı:

```json
{
  "distribution_mode": "sdk_relay",
  "targets": [
    {
      "platform": "youtube",
      "rtmp_url": "rtmp://a.rtmp.youtube.com/live2",
      "stream_key": "<encrypted-at-rest>",
      "enabled": true
    }
  ],
  "source_lang": "tr",
  "target_langs": ["en"],
  "overlay": {
    "position": "bottom",
    "font": "Inter",
    "size": 32
  }
}
```

Stream key'ler admin/dashboard API yanıtında SDK'ya TLS üzerinden düz gelir,
çünkü RTMP publish için gerekli. Diskte ve server-side persist katmanında
AES-256 ile şifreli tutulur; loglara asla yazılmaz.

## Google / YouTube Production Bağlantısı

Google tarafında iki ayrı kullanım var:

- Google Translate API key: `GOOGLE_TRANSLATE_KEY`
- YouTube OAuth client: `YOUTUBE_CLIENT_ID`, `YOUTUBE_CLIENT_SECRET`

### Google Cloud Project

1. Google Cloud Console'da production için ayrı proje aç.
2. APIs & Services bölümünde gereken API'leri enable et:
   - YouTube Data API v3 / YouTube Live Streaming API
   - Cloud Translation API veya kullanılan Google Translate endpoint'i
3. OAuth branding/consent bilgilerini doldur:
   - App name: StreamDubber
   - Support email
   - Homepage
   - Privacy Policy
   - Terms of Service
   - Authorized domain: production domain, örn. `strovak.com`

Google dokümanına göre external production app'lerde homepage, privacy policy
ve terms linkleri gerekir; kullanılan domainlerin authorized domain olarak
eklenmesi gerekir.

### YouTube OAuth Client

Google Cloud Console > APIs & Services > Credentials:

1. Create OAuth client.
2. Application type: Web application.
3. Authorized redirect URI ekle:

```text
https://api.strovak.com/api/integrations/youtube/callback
```

Staging/local için ayrı client veya ayrı redirect ekle:

```text
http://localhost:8080/api/integrations/youtube/callback
```

4. Client ID ve Client Secret değerlerini admin panelde gir:

```text
Admin > Secrets > YOUTUBE_CLIENT_ID
Admin > Secrets > YOUTUBE_CLIENT_SECRET
```

5. Admin config'te redirect base production domain olmalı:

```text
integration.redirect_base = https://api.strovak.com/api/integrations
```

Google redirect URI birebir eşleşme ister. Şema (`https`), domain, path ve
trailing slash farklıysa `redirect_uri_mismatch` alınır.

Kod tarafında YouTube için gerekenler hazır olmalı:

```text
scope = https://www.googleapis.com/auth/youtube.force-ssl
access_type = offline
prompt = consent
```

`access_type=offline` refresh token almak içindir. Refresh token, yayıncı
panelde YouTube hesabını bir kere bağladıktan sonra tekrar consent istemeden
caption API çağrılarının sürdürülebilmesini sağlar.

### YouTube Connect Smoke

Operator akışı:

1. Admin > Secrets: `YOUTUBE_CLIENT_ID` ve `YOUTUBE_CLIENT_SECRET` gir.
2. Admin > Secrets veya provider satırında Test varsa çalıştır.
3. Yayıncı panelinde YouTube Connect'e bas.
4. Google consent ekranında doğru app adı ve domain görünüyor mu kontrol et.
5. Callback sonrası platform `connected` görünmeli.
6. Gerçek canlı yayında YouTube Live CC smoke yapılmalı.

## Google/Twitch Login (Sign-in) OAuth İstemcileri

Bunlar **YouTube OAuth Client**'tan (yukarıda) ayrıdır — o istemci yayıncının
kendi kanalına yayın/caption yazma iznidir (broadcast bağlama, plan gerektirir),
buradaki ise "Continue with Google/Twitch" ile hesaba giriş kimliğidir (bkz.
`backend/src/auth/oauth_login.rs` modül dokümanı). İkisini karıştırıp aynı
istemciyi kullanmak, girişte yayıncıya gereksiz geniş bir consent ekranı
gösterir ("kanalını yönetme izni ver, sadece giriş yapmak için").

Callback backend'e gider (`api.strovak.com`), frontend'e (`strovak.com`) değil
— `/api/auth/oauth/{provider}/callback` bir Axum route'u, Next.js sayfası
değil. Yanlışlıkla frontend domain'i veya `/auth/{provider}/callback` gibi
kısaltılmış bir path girmek `redirect_uri_mismatch` ile sonuçlanır.

### Google Login Client

1. Google Cloud Console > APIs & Services > Credentials > Create OAuth client
   (ayrı bir client — YouTube client'ıyla paylaşma).
2. Application type: Web application.
3. Authorized redirect URI:

```text
https://api.strovak.com/api/auth/oauth/google/callback
```

Staging/local için ayrıca:

```text
http://localhost:8080/api/auth/oauth/google/callback
```

4. OAuth consent screen (branding) — bu YouTube client'ıyla PAYLAŞILIR, proje
   başına tektir: Authorized domain `strovak.com` (alt domain `api.strovak.com`
   otomatik kapsanır), Homepage/Privacy/Terms linkleri `strovak.com` altında.
5. Client ID/Secret admin panelden:

```text
Admin > Secrets > GOOGLE_LOGIN_CLIENT_ID
Admin > Secrets > GOOGLE_LOGIN_CLIENT_SECRET
```

### Twitch Login Client

1. Twitch Developer Console > Register Your Application.
2. OAuth Redirect URLs:

```text
https://api.strovak.com/api/auth/oauth/twitch/callback
http://localhost:8080/api/auth/oauth/twitch/callback
```

3. Client ID/Secret admin panelden:

```text
Admin > Secrets > TWITCH_CLIENT_ID
Admin > Secrets > TWITCH_CLIENT_SECRET
```

### Redirect Base (her iki sağlayıcı için ortak)

Backend authorize URL'i kurarken `auth.oauth_redirect_base` config'inden
`redirect_uri`'yi türetir (varsayılan `http://localhost:8080/api/auth/oauth`,
dev içindir). Production'da admin config'te:

```text
auth.oauth_redirect_base = https://api.strovak.com/api/auth/oauth
integration.frontend_base = https://strovak.com
```

Google/Twitch redirect URI birebir eşleşme ister — şema, domain, path,
trailing slash farklıysa `redirect_uri_mismatch` alınır. `integration.frontend_base`
zaten yayın-hedefi bağlama akışıyla paylaşılan aynı config anahtarı; login
callback'i de başarı/hata sonrası buraya (`/login/oauth-complete` veya hesap
linkleme için `/dashboard/account`) 302 ile döner.

## Stripe Production Bağlantısı

1. Stripe Dashboard'da ürün/fiyatları oluştur:
   - free
   - starter
   - pro
   - streamer
2. Admin > Secrets:

```text
STRIPE_SECRET_KEY
STRIPE_WEBHOOK_SECRET
```

3. Stripe webhook endpoint:

```text
https://api.strovak.com/webhooks/stripe
```

4. Admin config:

```text
billing.plan.free.stripe_price_id
billing.plan.starter.stripe_price_id
billing.plan.pro.stripe_price_id
billing.plan.streamer.stripe_price_id
```

5. Test:
   - Admin provider Test: Stripe
   - gerçek test checkout
   - webhook delivery success
   - subscription DB persist

## Streamlabs Production Bağlantısı

1. Streamlabs developer/app ekranında production app oluştur.
2. Callback URL:

```text
https://api.strovak.com/api/integrations/streamlabs/callback
```

3. Admin > Secrets:

```text
STREAMLABS_CLIENT_ID
STREAMLABS_CLIENT_SECRET
```

4. Yayıncı panelinden Streamlabs Connect smoke yapılır.
5. Token'ın `connected` durumuna geçtiği, diskte düz token yazılmadığı ve
   gerçek Streamlabs API çağrısının çalıştığı doğrulanır.

## Provider Key Smoke

Admin > Secrets ekranında gir ve Test butonuyla doğrula:

| Secret | Test sonucu |
| --- | --- |
| `DEEPGRAM_KEY` | Deepgram project/list endpoint OK |
| `ASSEMBLYAI_KEY` | AssemblyAI endpoint OK |
| `DEEPL_KEY` | DeepL usage/translate OK |
| `GOOGLE_TRANSLATE_KEY` | Google languages endpoint OK |
| `STRIPE_SECRET_KEY` | Stripe balance OK |

Test geçmeden `providers.mock_fallback=false` yapılmamalı. Testler geçtikten
sonra production'da mock fallback kapatılır:

```text
providers.mock_fallback = false
```

## Canlı Yayın Smoke Checklist

Operator'ın MVP öncesi yapacağı canlı test sırası:

1. Admin login çalışıyor.
2. Secrets maskeli görünüyor; tam değer API/DOM'da görünmüyor.
3. Provider Test butonları geçiyor.
4. Yayıncı paneli SDK pairing kodu üretiyor.
5. SDK pairing yapıyor.
6. RTMP hedefleri web panelden giriliyor.
7. SDK hedefleri çekiyor.
8. OBS SDK localhost RTMP listener'a yayın basıyor.
9. SDK YouTube/Twitch/Kick hedeflerine fan-out ediyor.
10. Overlay OBS Browser Source'ta görünüyor.
11. Latency 2 saniyenin altında ölçülüyor.
12. Stripe checkout ve webhook çalışıyor.
13. Logs ekranında hata yok; stream key/API key sızıntısı yok.

## Kaynaklar

- Google OAuth web server flow:
  https://developers.google.com/identity/protocols/oauth2/web-server
- YouTube Data API OAuth web app flow:
  https://developers.google.com/youtube/v3/guides/auth/server-side-web-apps
- Google OAuth branding / authorized domains:
  https://support.google.com/cloud/answer/15549049
