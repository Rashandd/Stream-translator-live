# Caption Overlay Builder & Streamer Compatibility Plan

## Neden

- `frontend/app/o/[token]/page.tsx` overlay renderer'ı gerçek, çalışan kod: WebSocket'e bağlanıyor,
  reconnect ediyor, backend'de e2e test edilmiş (`tests/overlay_api.rs`), ve `?lang=&font=&size=&color=&bg=&position=`
  query param'larıyla zaten özelleştirilebiliyor.
- `frontend/app/[locale]/dashboard/captions/page.tsx`'deki "Caption appearance" bölümü statik bir
  `label: value` listesi (`captions.appearance` dizisi) — hiçbir input yok, "Live preview" sabit
  bir örnek cümle gösteriyor. Overlay'in **çalışan** tarafı var, onu **hazırlayacak panel** yok.
- Site genelinde 22 ayrı metin "OBS"'e özel yazılmış (aşağıda tam liste), ama Browser Source
  overlay yöntemi herhangi bir Browser/Web Source destekleyen yazılımda (Streamlabs Desktop,
  XSplit, vMix) ve herhangi bir OS'te çalışır — metin bunu yansıtmıyor, ürünü OBS'e kilitli
  gösteriyor.

## Kapsam kararları

- Mobil-sadece yayıncı (telefon üzerinden native yayın) desteklenmeyecek — plana dahil değil.
- SDK'nın OBS'e obs-websocket ile otomatik altyazı kaynağı eklemesi (`sdk/src/integrations/obs.rs`,
  şu an bağlanmamış scaffold) bu planda YOK — manuel "URL'i kopyala → Browser Source'a yapıştır"
  yeterli kabul edildi.
- Mac/Linux ses yakalama (`sdk/src/audio/{coreaudio,pipewire}.rs`) bu planın kapsamı dışında; ayrı
  bir backend/SDK roadmap kalemi (`streamdubber_roadmap_final.md`). Bu plan yalnızca **frontend**:
  (1) yeni bir Overlay Builder paneli, (2) OBS'e kilitli metinlerin yazılım-agnostik hale getirilmesi.
- Bu turda backend wiring yok — dashboard'un geri kalanı gibi (bkz. `frontend_onboarding_audit_2026-07-04`
  memory notu) Overlay Builder da client-side kalıyor, gerçek bir stream token'ı veya API çağrısı
  gerektirmiyor. **Tek istisna:** §6'daki waitlist CTA'sı — bir email'i gerçekten bir yere kaydetmesi
  gerektiği için (dekoratif bırakılırsa lead yakalama amacının kendisi anlamsızlaşır) minimal bir
  backend endpoint'i bu planın parçası.
- Faz 2/3 kataloğunun (dubbing/voice cloning, Zoom/Teams/Meet, Watch Party Mode, sektörel sayfalar
  vb.) admin panelden DB-driven toggle edilmesi bu planda YOK (kullanıcı onayı: "sadece coming soon
  olarak hazırlanıp tanıtılması yeter") — §6'daki genişleme de mevcut statik `en.json` konvansiyonunu
  kullanıyor, roadmap'in öngördüğü tam DB/admin-config sistemi ayrı, daha büyük bir iş olarak kalıyor.
- Konsol (PS5/Xbox) + capture card ile görüntüyü/sesi PC'ye alıp OBS/Streamlabs Desktop'tan yayına
  giren yayıncılar da bu planın kapsamında: Strovak'ın gördüğü şey capture card sonrası PC'de
  çalışan bir OBS'ten farksız olduğu için overlay builder'ın ürettiği Browser Source URL'i onlar
  için de aynı şekilde çalışır — ayrı bir teknik iş gerekmiyor, sadece §3'teki metinlerde bu kitleyi
  dışlar gibi okunan "PC" vurgusunu düzeltiyoruz ve bir yerde açıkça güvence veriyoruz.

## 1. Teknik düzeltme: canlı önizleme gerçek token gerektirmemeli

İlk taslakta "gerçek `/o/{token}` URL'ini iframe'e göm" önerilmişti — ama dashboard'da henüz gerçek
bir overlay token'ı yok (backend wiring bekliyor, bkz. `captions.overlayUrl` şu an maskeli:
`"https://overlay.strovak.com/o/•••••••"`). Gerçek olmayan bir token'la iframe açmak WebSocket
bağlantısı kuramaz, hiçbir altyazı gelmez, önizleme boş kalır — yani "canlı" önizleme fikri bu
aşamada teknik olarak çalışmaz.

**Çözüm — tek doğruluk kaynağı, paylaşılan stil fonksiyonu:**
- Yeni dosya: `frontend/src/lib/overlay-style.ts` — `/o/[token]/page.tsx`'teki inline stil mantığını
  (font/size/color/bg/position → React `CSSProperties`) buraya taşı, saf bir fonksiyon olarak:
  `computeOverlayStyle({ font, size, color, bg, position }): { container: CSSProperties; caption: CSSProperties }`.
- `frontend/app/o/[token]/page.tsx` bu fonksiyonu import edip kullanacak şekilde refactor edilir
  (davranış değişmez, sadece stil hesaplama tek yerde toplanır).
- Yeni Overlay Builder da aynı fonksiyonu kullanarak **yerel** bir `<div>` içinde örnek bir altyazı
  cümlesiyle ("Merhaba, bu senin çevrilmiş altyazın." gibi çift dilli örnek) önizleme gösterir —
  iframe/WebSocket yok, sadece aynı stil formülünün canlı uygulanması. Böylece iki render yolu
  arasında hiçbir zaman sapma (drift) olmaz, çünkü ikisi de aynı fonksiyonu çağırıyor.
- Backend wiring geldiğinde (ayrı bir iş), builder gerçek token varsa isteğe bağlı olarak iframe'e
  geçebilir — bu plan o geçişi kapsamıyor, sadece yolu açık bırakıyor.

## 2. Caption Overlay Builder (yeni panel)

Dosya: `frontend/app/[locale]/dashboard/captions/page.tsx` — statik "Caption appearance"
`SectionCard`'ı interaktif hale getir. Sayfa kendisi server component kalır (i18n/config yükleme
aynı kalır); yeni interaktif kısım ayrı bir client component'e taşınır.

Yeni client component: `frontend/src/components/caption-overlay-builder.tsx` (`"use client"`).
Props: `{ overlayUrlTemplate: string; targets: { code: string; language: string; enabled: boolean }[] }`
(config'ten geçilir, mevcut `captions.overlayUrl` ve `captions.targets` kullanılır).

**Kontroller ve varsayılanlar** (varsayılanlar `/o/[token]/page.tsx`'teki mevcut query-param
varsayılanlarıyla birebir eşleşir — kullanıcı hiçbir şeye dokunmasa bile üretilen URL bugünkü
overlay davranışıyla aynı sonucu verir):

| Kontrol | Tip | Seçenekler | Varsayılan |
|---|---|---|---|
| Dil | select | `targets` içinden `enabled: true` olanlar + "Kaynak dil (çeviri yok)" | Kaynak dil |
| Font | select | `Inter, system-ui, sans-serif` / `Arial, sans-serif` / `Georgia, serif` / `"Courier New", monospace` — bunlar bundled webfont değil, düz CSS `font-family` string'i; OBS'in kendi Chromium'u OS'teki sistem fontlarıyla render eder, bu yüzden hangi OS/OBS-alternatifinde olursa olsun çalışır | `Inter, system-ui, sans-serif` |
| Boyut | select | S=`24px`, M=`32px`, L=`44px` | M (`32px`) |
| Yazı rengi | `input type="color"` + hex metin senkron | — | `#ffffff` |
| Arkaplan | `input type="color"` (renk) + ayrı opaklık slider (0–100) → `rgba(r,g,b,opaklık)` üretir | — | `rgba(0,0,0,0.6)` |
| Pozisyon | iki butonlu toggle | Üst / Alt (`position=top\|bottom`) | Alt |

- Canlı önizleme: yukarıdaki §1'deki `computeOverlayStyle` ile aynı anda render edilen örnek metin.
- Çıktı: `overlayUrlTemplate` + üretilen query string, mevcut copy-row görsel deseniyle (bkz. aşağıda
  "CopyRow ortaklaştırma") kopyalanabilir. Token gerçek olmadığı için URL örnek/maskeli kalmaya
  devam eder — bu bilinen bir sınırdır, ayrı not olarak UI'da küçük bir açıklama satırı gösterilir
  ("Gerçek yayın token'ın hazır olduğunda bu URL otomatik güncellenecek" gibi, config'ten gelen metin).
- State sadece component içinde (`useState`); kalıcı saklama yok.

**CopyRow ortaklaştırma:** `go-live/page.tsx` içinde yerel bir `CopyRow` fonksiyonu var,
`captions/page.tsx` ve `sdk/page.tsx` aynı deseni satır satır kopyalayarak inline yazmış (3 farklı
yerde neredeyse aynı JSX). Bu iş kapsamında bu deseni `frontend/src/components/copy-row.tsx`'e
çıkar, dört kullanım noktasının (3 mevcut + yeni builder) hepsini ona yönlendir — CLAUDE.md'nin
"üç benzer satır prematüre soyutlamadan iyidir" ilkesiyle çelişmiyor çünkü zaten var olan
kopyaları birleştiriyoruz, yeni bir spekülatif soyutlama eklemiyoruz.

## 3. Yazılım/OS-agnostik metin geçişi — tam liste (22 satır, `frontend/src/i18n/dictionaries/en.json`)

İlke: bir sayfada ilk "OBS" geçişi somut örnek listesiyle genişletilir ("OBS, Streamlabs Desktop,
XSplit, vMix veya Browser/Web Source destekleyen herhangi bir yazılım"); aynı sayfadaki sonraki
geçişler kısaca "yayın yazılımın" / "sahnen" gibi genel ifadeye iner (her cümlede uzun listeyi
tekrar etmek okunabilirliği bozar).

| Path | Şimdi | Öneri |
|---|---|---|
| `home.hero.summary` | "...captions baked into OBS..." | "...captions baked into your scene..." |
| `home.scene.items[0]` | "Add translated captions directly into your OBS scene." | "Add translated captions directly into your scene — works with OBS, Streamlabs Desktop, XSplit, vMix, or anything with a Browser Source." |
| `home.now.items[1].body` | "...baked into OBS..." | "...baked into your scene..." |
| `home.edge.items[0].title` | "Captions baked into OBS" | "Captions baked into your scene" |
| `home.edge.items[0].body` | "...burned into your OBS scene..." | "...burned into your scene in OBS, Streamlabs Desktop, XSplit, vMix, or any Browser Source-capable software..." |
| `dashboard.overview.howItWorks.steps[1].body` | "Baked into your OBS scene..." | "Baked into your scene..." |
| `dashboard.distribution.subtitle` | "Embedded OBS captions work today..." | "Embedded captions work today in any streaming software..." |
| `dashboard.distribution.platformSupport.note` | "Embedded OBS captions already work..." | "Embedded captions already work in OBS, Streamlabs Desktop, XSplit, vMix, or any Browser Source-capable software..." |
| `dashboard.goLive.subtitle` | "Strovak never replaces OBS..." | "Strovak never replaces your streaming software..." |
| `dashboard.goLive.modes.relay.name` | "Baked into your stream (OBS)" | "Baked into your stream" |
| `dashboard.goLive.modes.relay.summary` | "OBS stays where you run the show...your gameplay and video never leave your PC..." | "Your streaming software stays where you run the show...your gameplay and video never leave your setup — same if you're on a gaming PC or routing a console through a capture card..." |
| `dashboard.goLive.modes.relay.steps[1].title` | "Add captions to OBS" | "Add captions to your scene" |
| `dashboard.goLive.modes.relay.steps[1].body` | "...add it in OBS as a Browser source..." | "...add it as a Browser/Web Source in OBS, Streamlabs Desktop, XSplit, vMix, or your streaming software — works the same whether you stream straight from your PC or route a console through a capture card into OBS." (bu ilk teknik geçiş — tam liste + konsol/capture-card güvencesi burada verilir) |
| `dashboard.goLive.modes.relay.steps[3].title` | "Start in OBS, then arm translation here" | "Start streaming as usual, then arm translation here" |
| `dashboard.goLive.modes.relay.steps[3].body` | "Press Start Streaming in OBS..." | "Press Start Streaming in your software..." |
| `dashboard.goLive.modes.relay.connectLabel` | "...add as a Browser source in OBS" | "...add as a Browser/Web Source in your streaming software" |
| `dashboard.goLive.modes.relay.videoNote` | "Video is encoded and sent by OBS..." | "Video is encoded and sent by your streaming software..." |
| `dashboard.goLive.modes.relay.startNote` | "...keeps flowing through OBS..." | "...keeps flowing through your streaming software..." |
| `dashboard.goLive.modes.cloud.summary` | "OBS sends one clean stream..." | "Your streaming software sends one clean stream..." |
| `dashboard.goLive.modes.cloud.steps[2].title` | "Point OBS at Strovak" | "Point your streaming software at Strovak" |
| `dashboard.goLive.modes.cloud.steps[2].body` | "...into OBS stream settings." | "...into your streaming software's stream settings." |
| `dashboard.goLive.modes.cloud.connectLabel` | "...paste into OBS" | "...paste into your streaming software" |
| `security.claims[0].body` | "...delivered by OBS from your PC..." | "...delivered by your streaming software from your setup — PC or console via capture card..." |

Dokunulmayacaklar: `/dashboard/sdk` platform durum pilleri (zaten config-driven, OS-agnostik) ve
SDK'ya özgü ses-yakalama metinleri (`sdk.*` anahtarları) — onlar OBS'e değil, işletim sistemine
referans veriyor, kapsam dışı.

## 4. Test etkisi

- `frontend/src/i18n/dictionaries/marketing.test.ts:45` şu an `expect(homeText).toContain("captions baked into obs")` — bu satır `home.edge.items[0].title` metnine bağlı, metin değişince
  `"captions baked into your scene"` gibi yeni beklenene güncellenmeli.
- Yeni `caption-overlay-builder.tsx` için component test: her kontrolün query string'e doğru
  yansıdığını (özellikle renk→hex, opaklık→rgba, pozisyon→top/bottom) ve varsayılan durumda
  `/o/[token]/page.tsx`'in bugünkü varsayılanlarıyla birebir aynı sonucu ürettiğini doğrulayan
  testler.
- `overlay-style.ts` için birim test: aynı girdiyle hem builder hem `/o/[token]` aynı stil objesini
  üretiyor mu (drift'i engelleyen asıl garanti budur).

## 5. Favicon ve sekme (tab) başlıkları

İki ayrı doğrulanmış eksik (ilk izlenim/anlaşılabilirlik ile doğrudan ilgili, bu yüzden aynı plana
eklendi):

**Favicon yok.** `frontend/app/` ve `frontend/public/` içinde `favicon.ico`/`icon.png`/`icon.svg`
aranıp bulunamadı — sadece `public/logo-strovak.png` ve `logo-strovak.original.png` var, ikisi de
kare değil (tam boy logo). Next.js App Router `app/icon.png` (veya `.ico`/`.svg`) dosyasını otomatik
favicon olarak sunar. **Yapılacak:** mevcut `logo-strovak.png` markasından kare kırpılmış bir ikon
üretip `frontend/app/icon.png` olarak ekle. Not: logonun kare bir kırpımı yoksa (yatay/dikdörtgen
bir wordmark ise) otomatik kırpma anlamsız bir favicon üretebilir — bu durumda tek başına marka
işaretini (sadece "S" harfi/simge gibi) içeren ayrı bir kare asset gerekebilir; bu plan bunu bir
varsayım olarak not ediyor, gerekirse kullanıcıdan hazır bir kare ikon istenecek.

**Her sekme aynı başlığı gösteriyor.** Tüm proje genelinde `metadata` export'u sadece
`frontend/app/layout.tsx`'te var (`title: en.brand.name`, statik) — hiçbir route'ta
`generateMetadata` veya kendi `metadata` yok. Sonuç: landing, pricing, solutions, security,
signup, login, sekiz dashboard sayfası, yedi admin sayfası — hepsi tarayıcıda birebir aynı sekme
başlığını gösteriyor. Doğrulanan durum: **her sayfa zaten kendi başlık string'ine sahip**
(`PageHeader`'a geçirilen `title` prop'u, ör. `sdk.title`, `captions.title`, `productionSetup.title`,
`auth.signup.title="Create your account"`, marketing sayfalarında `*.hero.eyebrow` — ör.
`pricing.hero.eyebrow="Pricing"`) — yani yeni metin yazmaya gerek yok, var olan başlığı
`<title>`'a bağlamak yeterli.

**Yapılacak:**
- `frontend/app/layout.tsx`: `metadata.title`'ı template'e çevir —
  `title: { template: "%s · Strovak", default: en.brand.name }`.
- Her locale-scoped route'a (`app/[locale]/**/page.tsx`) `generateMetadata({ params })` ekle —
  sayfanın zaten yaptığı `getPublicConfig(locale)` çağrısını tekrar kullanarak, o sayfanın var olan
  `title`/`hero.eyebrow` alanını döndürsün. Örnek (`/dashboard/sdk`):
  ```ts
  export async function generateMetadata({ params }: Props): Promise<Metadata> {
    const { locale: rawLocale } = await params;
    if (!isLocale(rawLocale)) return {};
    const config = await getPublicConfig(rawLocale);
    return { title: config.dashboard.sdk.title };
  }
  ```
- Kapsanan sayfalar: landing, `pricing`, `solutions`, `security`, `signup`, `login`, `login/reset`,
  `login/verify`, dashboard'un 8 sayfası, admin'in 7 sayfası (+admin login/verify).
- Kapsam dışı: `/o/[token]` overlay sayfası — OBS Browser Source içinde görünmez şekilde çalışır,
  kimse sekme başlığına bakmaz, bu yüzden gereksiz.

## 6. Faz 2/3 kataloğunu "Coming Soon" olarak tanıt + waitlist CTA'sı

Şu an `/solutions` sayfası roadmap'in "Coming Soon — Palabra Benzeri Çözüm Kataloğu" bölümünde
tarif ettiği kalemlerin sadece bir kısmını içeriyor (6 madde: live stream translation, voice
dubbing, webinar, events, online meetings, developer API). Roadmap'te ayrıca sayılan ama şu an
`solutions.items`'ta hiç olmayan kalemler: Zoom/Teams/Google Meet'e özel çeviri modları, Watch
Party Mode, ve sektörel sayfalar (Churches, Education, Nonprofit, Broadcasting, International
Companies). Kullanıcı kararı: bunların hepsinin **admin panelden DB-driven toggle edilmesine**
gerek yok (bu ayrı, büyük bir iş) — sadece hepsinin frontend'de `coming_soon`/`waitlist` olarak
**tanıtılması** yeterli, ki yayıncı ürünün tüm vizyonunu görsün; birini "satın almak" isteyince
email bırakıp ilk teklif/erken erişim listesine girebilsin.

**Yapılacak — içerik:**
- `frontend/src/i18n/dictionaries/en.json`'daki `solutions.items` dizisine roadmap'in kalan
  kalemlerini ekle, her biri mevcut `{ title, body, icon, audience, status }` şeklinde:
  - "Zoom translation", "MS Teams translation", "Google Meet translation" (`status: "coming_soon"`, `audience: "business"`)
  - "Watch Party Mode" (`status: "coming_soon"`, `audience: "creators"` — izleyici tarafı bir mod)
  - "Churches", "Education", "Nonprofit", "Broadcasting", "International companies" (`status: "waitlist"`, ilgili `audience`)
  - Aynı mantık `home.edge.items`'a da uygulanabilir ama orası zaten yer kısıtlı bir özet — asıl
    genişleme `/solutions` sayfasında, landing sadece oraya link verir (mevcut `home` → `/solutions`
    CTA akışı zaten var).

**Yapılacak — waitlist CTA (frontend + minimal backend):**
- Yeni paylaşılan client component: `frontend/src/components/waitlist-cta.tsx` — `status !== "enabled"`
  olan her kart için ortak bir "Yayınlandığında haber ver" butonu + email input + submit. Başarı/hata
  durumu gösterir (dashboard'un geri kalanının aksine bu **gerçekten** çalışmalı, çünkü amacı lead
  toplamak — dekoratif bırakmak özelliğin amacını ortadan kaldırır).
- `/solutions` sayfasındaki her `coming_soon`/`waitlist` kartına bu component eklenir, `interest`
  prop'u olarak o kartın `title`'ı (veya ayrı bir `key` alanı) geçirilir.
- Backend: yeni minimal endpoint — `backend/src/waitlist.rs` (mevcut `backend/src/pairing.rs` gibi
  tek-sorumluluk modül konvansiyonuna uygun), `POST /api/waitlist { email, interest }` → Postgres'e
  yazar. Yeni migration: `backend/migrations/0002_waitlist.sql` (`waitlist_signups`: `id`, `email`,
  `interest`, `created_at`; `email`+`interest` üzerinde unique constraint — aynı kişi aynı özelliğe
  iki kez kaydolamaz). Route `backend/src/app.rs`'e `.route("/api/waitlist", post(waitlist::submit))`
  olarak eklenir. Auth gerekmiyor (herkese açık lead-capture), ama email format validasyonu +
  basit rate-limit (aynı IP'den dakikada N istek) CLAUDE.md güvenlik ilkeleriyle tutarlı asgari
  önlem. `unwrap()` yasak kuralına uy, `?` ile error propagation.
- Frontend çağrısı `"use client"` component içinde doğrudan `NEXT_PUBLIC_API_URL`'ye (`api.strovak.com`)
  gider — CLAUDE.md'nin "Frontend API Proxy YASAK" kuralına uygun, `src/lib/api.ts`'teki mevcut
  fetch deseni takip edilir.

**Kapsam dışı (bu maddede de):** admin panelden waitlist kayıtlarını görüntüleme/CSV export gibi bir
arayüz — bu plan sadece toplama tarafını kapsıyor, operatör tarafı (kayıtları görme) ayrı bir iş.

**Test etkisi:** backend'de `cargo test` — geçerli email + yeni interest → 201, aynı email+interest
tekrarı → unique constraint hatası temiz şekilde ele alınıyor (500 değil, anlamlı bir hata/duplicate
yanıtı), geçersiz email formatı → 400. Frontend'de `waitlist-cta.tsx` için submit/success/error
durumlarını doğrulayan bir component test.

## Kapsam dışı bırakılanlar (ileride tekrar sorulmasın diye kayıt altında)

- Mobil yayıncı desteği.
- obs-websocket otomatik entegrasyonu.
- Mac/Linux SDK ses yakalama (ayrı roadmap kalemi).
- Faz 2/3 kataloğunun admin panelden DB-driven toggle edilmesi (roadmap'in tam öngördüğü sistem) —
  bu planda sadece statik "coming soon" tanıtımı + waitlist var, tam toggle sistemi ayrı bir iş.
- Waitlist kayıtlarını admin panelden görüntüleme arayüzü.
- Faz 3 "bulut multistream" (Mod C, zayıf internetli yayıncılar için premium Hetzner/MediaMTX
  seçeneği, `docs/multistream.md`) §6'daki coming-soon kataloğuna kasıtlı olarak eklenmedi —
  roadmap'in en sonunda kalan, düşük öncelikli bir kalem; dublaj (Faz 2) ondan bağımsız zaten
  yayıncının kendi SDK relay'i üzerinden çalışıyor, bu yüzden frontend'de şimdilik hiç öne çıkarılmıyor.
- Gerçek token'la canlı iframe önizleme (yol açık bırakıldı, bu planın parçası değil).

## Doğrulama

- `npm run lint`, `npm run test`, `tsc`.
- `/dashboard/captions`'ı aç, her kontrolü değiştir, üretilen query string'i doğrudan `/o/{herhangi-bir-token}?...` olarak tarayıcıda aç, `overlay-style.ts` üzerinden render edilen görünümün
  builder önizlemesiyle piksel piksel aynı olduğunu doğrula.
- `marketing.test.ts` ve yeni eklenen testler yeşil.
- Tarayıcıda birkaç farklı route'u sekmede aç (ör. landing, `/dashboard/sdk`, `/admin/production-setup`),
  her birinin sekme başlığının farklı ve doğru olduğunu, favicon'un göründüğünü gözle doğrula.
