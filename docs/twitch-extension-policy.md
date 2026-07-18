# Twitch Extension Policy ve Uyumluluk Dokümanı

Durum: Kabul edildi  
Kapsam: StreamDubber Twitch Extension, EBS, viewer UI, broadcaster config, dashboard entegrasyonu  
Faz: Faz 2 hazırlığı; Faz 1 içinde implement edilmez  
Son resmi kaynak kontrolü: 2026-07-03

## Amaç

Bu doküman StreamDubber'ın Twitch Extension geliştirmesinde uyacağı ürün, teknik, güvenlik, gizlilik, review ve operasyon kurallarını tanımlar. Hedef, Twitch review sürecinde reddedilme riskini azaltmak ve Faz 2'de izleyici dil seçimi ile altyazı/dublaj deneyimini Twitch politikalarına uygun şekilde tasarlamaktır.

StreamDubber için Twitch Extension'ın ilk hedefi:

- İzleyicinin hedef dili seçmesi.
- Yayın üzerinde canlı çevrilmiş altyazı görmesi.
- Yayıncıya ait SDK ve backend pipeline'ı ile üretilen altyazı verisini güvenli şekilde alması.
- Viewer tarafında yazılım kurulumu gerektirmemesi.

Faz 2'de ses klonlama/dublaj eklenirse bu özellik ayrı bir policy gate'ten geçer. Extension içinde ses çalınacaksa varsayılan kapalı/mute olmalı ve izleyici ses seviyesini kontrol edebilmelidir.

## Resmi Kaynaklar

- Twitch Extensions Guidelines and Policies: https://dev.twitch.tv/docs/extensions/guidelines-and-policies/
- Twitch Building Extensions: https://dev.twitch.tv/docs/extensions/building/
- Twitch Designing Extensions: https://dev.twitch.tv/docs/extensions/designing/
- Twitch Life Cycle Management: https://dev.twitch.tv/docs/extensions/life-cycle/
- Twitch Submission Best Practices: https://dev.twitch.tv/docs/extensions/submission-best-practices/
- Twitch API in an Extension Front End: https://dev.twitch.tv/docs/extensions/frontend-api-usage/
- Twitch Extensions Reference: https://dev.twitch.tv/docs/extensions/reference/

## Faz Kararı

Twitch Extension Faz 1 kapsamı dışındadır. Faz 1 sadece ücretsiz altyazı, dashboard, SDK relay, backend pipeline ve opsiyonel Streamlabs/YouTube entegrasyonlarını hedefler.

Faz 2'ye geçilmeden önce şu koşullar tamamlanmış olmalıdır:

- Faz 1 caption pipeline stabil çalışır.
- Twitch kanal/account mapping modeli netleşir.
- Backend tarafında extension viewer traffic için rate-limit ve cache katmanı vardır.
- Extension EBS JWT doğrulama tasarımı uygulanmıştır.
- Review channel, test broadcaster hesabı ve test viewer hesapları hazırlanmıştır.
- Bu dokümandaki policy checklist "geçti" durumundadır.

## Ürün Konumu

Önerilen ilk Twitch Extension tipi: Video Overlay Extension.

Neden:

- Canlı altyazı doğrudan yayın içeriğiyle ilgilidir.
- İzleyici dil seçimi ve altyazı görünürlük kontrolleri interaktif bir deneyim sağlar.
- Panel extension altyazı için ana deneyim olmamalıdır; panel sadece yardım, durum veya ayar bağlantısı gibi tamamlayıcı işlerde kullanılabilir.

İlk release şu özelliklerle sınırlı olmalıdır:

- Dil seçimi.
- Altyazı aç/kapat.
- Altyazı boyutu veya konumu için sınırlı viewer ayarı.
- Bağlantı/durum göstergesi.
- Hata durumunda sade fallback mesajı.

İlk release'e alınmayacaklar:

- Viewer tarafından yüklenen metin, görsel, ses veya medya.
- Bits monetizasyonu.
- Reklam, sponsor içeriği veya üçüncü taraf kampanya alanları.
- Viewer'a uygulama indirtme.
- NFT, bahis, çekiliş, kumar veya rastgele ödül mekanikleri.

## Policy Kararları

### Yayıncı SDK Gereksinimi

Yayıncıdan StreamDubber SDK kurması istenebilir. Twitch politikaları broadcaster tarafında üçüncü parti yazılım gereksinimine izin verir.

Kurallar:

- SDK gereksinimi extension açıklamasında ve review walkthrough içinde açık yazılır.
- Review channel için SDK kurulu ve backend bağlantısı hazır olmalıdır.
- SDK hata verirse extension viewer tarafında çökmez; "stream captions unavailable" benzeri sade durum gösterir.

### Viewer Kurulumu Yasak

Viewer'dan StreamDubber SDK, Chrome extension, desktop app veya başka üçüncü parti yazılım kurması istenemez.

Kurallar:

- Viewer UI içinde "download", "install", "register to continue" gibi zorlayıcı akış yoktur.
- İzleyici dil seçimi anonim veya Twitch extension kimliğiyle çalışır.
- Viewer kimliği sadece gerekli olduğunda istenir.

### Altyazı ve İnteraktivite

Altyazı overlay'i yayın içeriğiyle ilgili olmalı ve izleyiciye anlamlı kontrol sağlamalıdır.

Zorunlu kontroller:

- Dil seçimi.
- Altyazı görünürlüğü aç/kapat.
- Okunabilirlik için en az bir ayar: boyut, kontrast veya konum.

Önerilen kontroller:

- Son birkaç altyazı satırını kısa süreli görüntüleme.
- Gecikme/durum göstergesi.
- Hata bildirimi veya feedback bağlantısı, off-site policy'ye uygun olacak şekilde.

### Audio ve Dublaj

Extension içinde çevrilmiş/dublaj ses çalmak ayrı riskli bir özelliktir. İlk Twitch Extension release'i subtitles-only olmalıdır.

Dublaj ses Faz 2'de eklenirse:

- Ses varsayılan olarak kapalı/mute başlar.
- Viewer'a açık volume/mute kontrolü verilir.
- Autoplay davranışı tarayıcı ve Twitch review ortamında ayrıca test edilir.
- Bits ile audio/VOIP-related işlev monetize edilmez.
- Ses çalma domainleri Media Domain Allowlist'e eklenir.
- Audio fallback: ses başarısızsa altyazı çalışmaya devam eder.

### Branding

Extension içinde Twitch adı, Twitch logosu, Glitch, global emote veya Twitch brand öğesi kullanılmaz.

Kurallar:

- Extension adı "StreamDubber Live Captions" gibi StreamDubber markalı olur.
- UI içinde "Twitch için resmi" gibi onay/ortaklık ima eden metin kullanılmaz.
- "Powered by StreamDubber" kullanılabilir; sponsor veya başka marka "brought to you by" şeklinde eklenmez.
- Screenshot ve discovery image gerçek extension UI'ını yansıtır.

### Reklam ve Sponsorluk

Extension içinde statik veya dinamik reklam gösterilmez.

Yasak:

- Sponsor logoları.
- Affiliate/referral linkleri.
- Ürün satın alma CTA'ları.
- StreamDubber plan satın alma CTA'sının viewer UI'da gösterilmesi.

Broadcaster config ekranında StreamDubber dashboard'a yönetim bağlantısı verilebilir; bu bağlantı açıkça off-site olduğunu göstermelidir.

### Off-site Linkler

Off-site linkler sınırlı tutulur.

İzinli örnekler:

- Broadcaster config içinde "Open StreamDubber dashboard" yönetim bağlantısı.
- Privacy Policy, Terms, Support bağlantıları.

Yasak veya yüksek riskli örnekler:

- Viewer UI'da kayıt ol, satın al, indir CTA'ları.
- Twitch'e rakip işlev sunan sayfalara link.
- Sosyal medya veya reklam/sponsor sayfaları.
- Extension işleviyle ilgisiz landing page.

### Commerce ve SaaS Planları

Broadcaster'a ücretli StreamDubber planları sunulabilir. Bu monetizasyon extension viewer UI içinde değil, StreamDubber dashboard ve billing akışında yönetilir.

Kurallar:

- Viewer extension içinde para, satın alma veya abonelik zorlaması yoktur.
- Plan limitleri DB/admin panelden gelir; extension bundle içine hard-coded plan/fiyat yazılmaz.
- Broadcaster planı yetersizse viewer'a ticari mesaj gösterilmez; broadcaster config/dashboard tarafında gösterilir.

### Bits

İlk release'te Bits tamamen kapalıdır.

Gelecekte Bits düşünülürse:

- Audio, VOIP, dublaj, track seçme veya sesli deneyim için Bits kullanılmaz.
- "donation", "support", "buy", "purchase", "spend", "cheer" gibi policy-riskli finansal dil kullanılmaz.
- Bits ürünleri broadcaster veya viewer serbest metnine bağlı olmaz.
- Ayrı Bits policy review yapılmadan kod yazılmaz.

### User Generated Content

İlk release viewer-submitted content içermez.

UGC sayılabilecek ve ayrı moderation gate gerektiren özellikler:

- Viewer altyazı düzeltmesi gönderir.
- Viewer çeviri önerir.
- Viewer metin, görsel, ses veya video yayınlar.
- Viewer feedback'i yayında veya extension içinde görünür hale gelir.

UGC eklenirse:

- Viewer Twitch ID paylaşımı istenir.
- Gönderen Twitch username'i gösterilir.
- Broadcaster approve/reject akışı kurulur.
- Broadcaster published content'i silebilir.
- Moderation API veya eşdeğer backend moderation kuyruğu kullanılır.

## Teknik Mimari

### Bileşenler

- Extension Frontend: Twitch CDN'de host edilen static HTML/CSS/JS bundle.
- Extension Backend Service (EBS): StreamDubber backend içinde ayrı extension API yüzeyi.
- StreamDubber Pipeline: SDK'dan gelen ses, STT, çeviri, caption state.
- Dashboard Config: broadcaster hesap eşleştirme, hedef diller, overlay ayarları.

### Frontend Paketleme

Extension frontend'i mevcut Next.js dashboard uygulamasının iframe'i veya remote sayfası olarak sunulmaz.

Kurallar:

- Ayrı, küçük, static extension package üretilir.
- Tüm frontend HTML/CSS/JS asset'leri review zip dosyasında bulunur.
- Kullanılmayan dosya, secret, source dışı artifact veya debug dosyası zip'e girmez.
- Twitch Extension Helper her HTML dosyasında ilk script olarak yüklenir.
- Flash, nested iframe ve keyboard shortcut tabanlı temel işlev yoktur.
- JS review edilebilir/human-readable kalır; obfuscation yoktur.
- `eval()` kullanan kütüphane varsa bundle'a gömülmez, ayrı source olarak açıklanır.
- `console.log` veya debug output commit edilmez.

### DOM ve Veri Güvenliği

Backend, Twitch API veya config servisinden gelen dinamik veri doğrudan DOM'a HTML olarak basılmaz.

Kurallar:

- Caption text `textContent` veya framework escaping ile render edilir.
- HTML injection, markdown render veya rich text yoktur.
- Dil kodları allowlist ile doğrulanır.
- URL'ler allowlist'ten seçilir; user-provided URL render edilmez.
- Hata mesajlarında secret, token, stream key veya provider response body sızdırılmaz.

### EBS Authentication

Extension frontend, Twitch Extension Helper `onAuthorized` akışından aldığı JWT'yi EBS çağrılarında header olarak gönderir.

EBS kuralları:

- Twitch extension JWT her istekte doğrulanır.
- JWT secret Vault/env kaynaklıdır; koda gömülmez.
- Shared secret rotasyonu desteklenir.
- Token expiry dikkate alınır; frontend en güncel token'ı kullanır.
- `role`, broadcaster/channel id ve viewer identity claim'leri doğrulanmadan yetki verilmez.
- Frontend'deki `viewer` objesi sadece UI koşulları için kullanılabilir; yetkili işlemde JWT backend'de doğrulanır.

### Viewer Identity

Dil seçimi için viewer identity şart değildir. Varsayılan model:

- Anonymous viewer: language preference local storage veya opaque id ile.
- Linked viewer: sadece kişiselleştirme, moderation veya cross-device preference gerekirse.

Identity link istenirse:

- Kullanıcıya değer açık anlatılır.
- Sadece gerekli anda istenir.
- Viewer ID, UGC/moderation veya preference dışında kullanılmaz.

### Configuration Service

Twitch Configuration Service yalnız public kabul edilebilecek ayarlar için kullanılır.

İzinli:

- Channel caption enabled flag.
- Default target language.
- Theme preset id.
- Required config version.
- StreamDubber channel binding reference id.

Yasak:

- API key.
- Stream key.
- JWT secret.
- OAuth refresh/access token.
- DeepL key.
- User private billing/plan details.

Broadcaster configuration tamamlanmadan extension aktif olduğunda viewer'a karışık hata yerine basit "not configured" durumu gösterilir. Gerekirse Twitch Required Configuration alanı kullanılarak aktivasyon config tamamlanana kadar engellenir.

### Realtime Caption Transport

İlk implementation için hedef transport:

- Extension frontend sadece allowlist'teki StreamDubber EBS domainleriyle konuşur.
- Caption state EBS'den kısa ömürlü, cache'lenebilir endpoint veya streaming transport ile alınır.
- Twitch PubSub, canlı altyazı akışı için ana taşıyıcı yapılmaz; resmi limitler kanal başına 1 mesaj/saniye ve 5 KB mesaj boyutu olduğu için altyazı temposu için yetersiz kalabilir.

Transport gate:

- Hosted Test içinde `wss://` kullanımının CSP/allowlist davranışı doğrulanmadan WebSocket zorunlu dependency yapılmaz.
- WebSocket review ortamında sorun çıkarırsa SSE veya kısa polling fallback zorunludur.
- Caption payload'ları küçük tutulur; geçmiş buffer'ı sınırlıdır.

### URL, CSP ve Domain Allowlist

Her version submission'da extension frontend'in eriştiği tüm URL/domainler listelenir.

Allowlist kategorileri:

- Image Domains: icon/screenshot dışında runtime image kullanılıyorsa.
- Media Domains: audio/TTS dosyası veya stream kullanılıyorsa.
- URL Fetching Domains: EBS API, caption transport, status endpoint.
- Panel URLs: varsa broadcaster/dashboard config linkleri.

Kurallar:

- Ortak/broad CDN kökü allowlist'e yazılmaz; unique bucket/domain kullanılır.
- Production, staging ve hosted test domainleri ayrı belirtilir.
- URL listesi her release changelog ile güncellenir.
- Viewer UI hiçbir zaman allowlist dışı üçüncü taraf endpoint'e fetch atmaz.

Örnek hedef domainler:

- `https://api.streamdubber.example`
- `https://extension.streamdubber.example`
- `wss://extension.streamdubber.example` yalnız Hosted Test doğrulaması sonrası

Gerçek domainler deploy kararından sonra güncellenecektir.

## Mobile ve Performans

Mobile support hedefleniyorsa:

- iOS ve Android Twitch app üzerinde test edilir.
- Initial page load Twitch CDN'den 1 MB altında tutulur.
- Yaklaşık 500 Kb/s bağlantıda 3 saniye altında yükleme hedeflenir.
- Portrait ve landscape chat viewport içinde tüm elementler taşmadan görünür.
- Tap target boyutu en az 44 pt/dp hedeflenir.
- Mobile UI sadece ana işleri gösterir: dil, aç/kapat, durum.

Genel performans kuralları:

- Büyük framework bundle'larından kaçınılır.
- Web-safe fontlar tercih edilir.
- Loading göstergeleri video deneyimini kapatmaz.
- Caption render frame drop yaratmayacak şekilde throttle edilir.
- Backend error durumunda aggressive retry yoktur; exponential backoff kullanılır.

## UI/UX Kuralları

Overlay:

- Video player control alanlarıyla çakışmaz.
- Collapsible veya hideable olur.
- Yayının ana içeriğini gereksiz kapatmaz.
- Broadcaster için konum/tema ayarı sunulur.
- Viewer için en az görünürlük ve dil kontrolü sunulur.

Accessibility:

- Yüksek kontrast.
- Okunabilir font boyutu.
- Renk körlüğünü zorlayan tek renk sinyalleri kullanılmaz.
- Text responsive şekilde taşmadan görünür.

State handling:

- Online: canlı captions.
- No captions yet: sessiz bekleme durumu.
- Pipeline delayed: küçük status göstergesi.
- Backend unavailable: retry + sade hata.
- Not configured: broadcaster config gerektiğini belirten non-commercial mesaj.
- Offline channel: overlay kendini pasifleştirir veya minimum durum gösterir.

## Güvenlik ve Gizlilik

StreamDubber ana güvenlik kararları bu extension için de geçerlidir:

- Video buluta çıkmaz.
- Buluta yalnız ses veya üretilmiş metin/caption state gider.
- Transit TLS 1.3.
- SDK-bulut hattında mTLS hedeflenir.
- Secret'lar Vault/env içinden gelir.
- At-rest hassas veri AES-256 ile korunur.
- Loglarda API key, stream key, JWT, OAuth token, ses verisi veya tam transcript sızdırılmaz.

Extension özel kuralları:

- EBS logları viewer id ve broadcaster id için minimum gerekli alanları tutar.
- Caption history kısa tutulur; product requirement yoksa kalıcı saklanmaz.
- Viewer preference silinebilir olmalıdır.
- Privacy Policy extension version details içinde güncel URL olarak bulunur.
- Terms/EULA ve support email review submission'da hazırdır.

## Backend Rate Limit ve Abuse Koruması

Extension viewer traffic yüksek fan-out oluşturabilir.

Zorunlu backend önlemleri:

- Per channel ve per viewer rate limit.
- Caption state cache.
- EBS endpoint'leri için request timeout.
- 429 ve 5xx için frontend backoff.
- Broadcaster plan limitleri viewer'a ticari mesaj olarak değil, backend policy olarak uygulanır.
- Abuse durumunda channel-level extension caption endpoint kapatılabilir.

## Review ve Lifecycle

Development aşamaları:

1. Local Test: extension package lokal geliştirilir.
2. Hosted Test: assets Twitch CDN'e yüklenir ve gerçek CSP/asset ortamında test edilir.
3. Review: tek version review'a gönderilir; değişiklik gerekirse Local Test'e dönülür ve queue sırası sıfırlanabilir.
4. Approved: release edilebilir.
5. Released: canlı version değiştirilemez; yeni değişiklik yeni version olarak review'a gider.

Review hazırlığında zorunlu bilgiler:

- Review channel URL.
- Walkthrough Guide.
- Change Log.
- Test broadcaster hesabı.
- Test viewer hesapları.
- StreamDubber dashboard test credential veya review mode.
- SDK kurulum adımları.
- Backend allowlist/domain listesi.
- Kaynak kod/bundle açıklaması.
- Kullanılan framework ve build komutları.
- Canlı review gerekiyorsa 09:00-17:00 PT uygun zaman blokları.

Review channel:

- Extension review sırasında load olmalı.
- SDK ve backend test ortamı hazır olmalı.
- Test yayınında örnek TR kaynak ses ve EN hedef altyazı senaryosu bulunmalı.
- Configuration dashboard hata vermeden tamamlanmalı.

## Submission Checklist

Version Details:

- Extension name StreamDubber markalı, Twitch brand kullanmıyor.
- Summary kısa ve doğru.
- Description tüm işlevleri doğru anlatıyor.
- Chat capability yoksa "Has Chat" kapalı.
- Icon, discovery image ve screenshot gerçek UI'ı gösteriyor.
- Privacy Policy, Terms/EULA, support email hazır.

Capabilities:

- Identity link varsayılan kapalı veya gerekçeli.
- Chat injection kapalı.
- Bits support kapalı.
- Configuration mode seçilmiş.
- Required configuration version belirlenmişse EBS ile set ediliyor.
- URL allowlist alanları eksiksiz.

Files:

- Viewer HTML.
- Config HTML.
- CSS.
- JS.
- Assetler.
- Extraneous dosya yok.
- Twitch helper ilk script.
- Human-readable JS veya source açıklaması.

Testing:

- Web normal/theater/fullscreen.
- Mobile iOS/Android.
- Portrait/landscape.
- Slow network.
- Backend unavailable.
- SDK disconnected.
- Caption burst.
- Long caption text.
- RTL veya uzun dil adları gerekiyorsa layout testi.
- Dark/light mode.

## Policy Gate Checklist

Her release review öncesi bu liste işaretlenmelidir:

- [ ] Faz 2 kapsamında mı? Faz 1'e sızmadı mı?
- [ ] Viewer kurulumu gerektirmiyor mu?
- [ ] Broadcaster SDK gereksinimi description ve walkthrough içinde açık mı?
- [ ] UI içinde Twitch brand/logo/emote yok mu?
- [ ] Reklam/sponsor/referral yok mu?
- [ ] Off-site linkler açık indicator içeriyor mu?
- [ ] Viewer purchase/download/register CTA yok mu?
- [ ] Audio varsa default mute ve volume control var mı?
- [ ] Bits kapalı mı?
- [ ] UGC yok mu? Varsa moderation gate tamam mı?
- [ ] Dynamic data DOM'a sanitize edilmeden basılmıyor mu?
- [ ] Tüm fetched/media/image URL'leri allowlist'te mi?
- [ ] Bundle 1 MB mobile hedefini koruyor mu?
- [ ] Load time 500 Kb/s'de 3 saniye altı mı?
- [ ] EBS Twitch JWT doğruluyor mu?
- [ ] Extension secret Vault/env üzerinden geliyor mu?
- [ ] Viewer identity yalnız gerekli olduğunda isteniyor mu?
- [ ] Caption fallback ve error state düzgün mü?
- [ ] Review channel, walkthrough ve changelog hazır mı?

## EBS JWT Tasarımı (A3 — uygulandı)

Bu bölüm, yukarıdaki "EBS Authentication" kurallarının koddaki karşılığını
sabitler. Uygulama: `backend/src/ext_twitch/jwt.rs` + `backend/src/ext_twitch/mod.rs`
(testler: `backend/tests/ext_twitch_api.rs`).

- Kütüphane: `jsonwebtoken` (zaten workspace bağımlılığı; kendi JWT'mizle aynı).
- Algoritma: HS256. Anahtar, Twitch console'un verdiği **base64** secret'tan
  `DecodingKey::from_base64_secret` ile türetilir.
- Secret kaynağı: `TWITCH_EXT_SECRET` — ManagedSecrets üzerinden (admin panel
  override → env/Vault taban). Her istekte çağrı anında çözülür; panelden
  girilen yeni secret restart'sız devreye girer. Koda gömülü değer yoktur.
- Rotasyon: `TWITCH_EXT_SECRET_PREVIOUS` ikinci aktif secret'tır. Doğrulama
  sırası: imza mevcut secret'la denenir; YALNIZ imza hatasında önceki
  secret'la bir kez daha denenir. `exp` hatası (imza geçerli) rotasyon
  denemesini kısa devre eder.
- Claim doğrulama sırası: imza → `exp` (leeway 0) → `channel_id` claim'i
  route'taki kanalla birebir eşleşme (yatay erişim koruması) → endpoint'e
  göre `role` (`bind` için `broadcaster` şart).
- `opaque_user_id` yalnız rate-limit anahtarıdır; kimlikleştirme yapılmaz,
  loglanmaz. Hata gövdeleri sabit metinlerdir; token/secret asla log'a ve
  yanıta yazılmaz.
- Viewer WS'inde token, tarayıcı `WebSocket` API'si başlık gönderemediği için
  `?token=` query parametresiyle taşınır; URL loglanmaz, token kısa ömürlüdür.

## Faz 2 Uygulama Planı

Önerilen sıra:

1. Policy checklist'i roadmap item'ı olarak kilitle.
2. Extension package iskeleti oluştur: viewer, config, shared UI.
3. EBS JWT doğrulama endpoint'lerini TDD ile ekle.
4. Broadcaster channel binding modelini dashboard'a bağla.
5. Caption state read endpoint veya streaming fallback tasarla.
6. Viewer language preference modelini ekle.
7. Hosted Test'te CSP, URL allowlist ve mobile davranışı doğrula.
8. Review Walkthrough Guide ve Change Log hazırla.
9. Review'a subtitles-only version gönder.
10. Dublaj/audio için ayrı experimental branch ve ayrı policy review başlat.

## Kesin Yasaklar

- Faz 1'de Twitch Extension implementasyonu.
- Viewer'a yazılım kurdurmak.
- Extension içinde Twitch markası/logo/emote kullanmak.
- iframe kullanmak.
- Obfuscated JS ile review'a gitmek.
- Dynamic AJAX verisini sanitize etmeden DOM'a basmak.
- Secret veya token'ı Twitch configuration service'e koymak.
- Viewer UI'da reklam, sponsor veya satın alma CTA'sı göstermek.
- Bits ile dublaj/audio/VOIP-related işlev sunmak.
- NFT, bahis, kumar, çekiliş veya rastgele parasal değer mekanikleri.
- Twitch review için canlı çalışmayan backend/SDK ortamı sunmak.
