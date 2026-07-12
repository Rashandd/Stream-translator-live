# Dashboard Spec

## Yayıncı Dashboard — Faz 1 Sayfaları
1. Ana Sayfa: canlı durum, hızlı başlat, bu ayki kullanım
2. Yayın Kurulumu: dağıtım modu (Mod A/C), platform OAuth bağlama
3. Dil ve Altyazı: konuşma dili, hedef diller, overlay URL
4. Platform Hesapları: YouTube/Twitch/Kick OAuth
5. Abonelik: plan, kullanım, Stripe yükseltme
6. SDK İndir: indirme + eşleşme kodu

## Admin Dashboard — Faz 1 Temeli
1. Genel Bakış: aktif yayın, latency, hata oranı
2. Kullanıcı Yönetimi: filtre, detay, plan değiştirme
3. Servis Toggle: STT/Çeviri/TTS provider config API üzerinden restart'sız geçiş
   (`PUT /admin/config`) — ayrı bir toggle sayfası yok (eski `/admin/routing`
   kaldırıldı, commit `00a7c90`)
4. Fiyatlandırma Yönetimi: plan fiyatları, limitler, özellikler DB'den
5. Maliyet Özeti: servis bazlı harcama

## Design & Runtime Config
- Frontend geliştirmeden önce repo kökündeki `DESIGN.md` okunur.
- Görsel yön: Palabra benzeri parlak, sakin, pill kontrollü, canlı çeviri odaklı ürün dili; ama Palabra asset/copy/marka öğeleri kopyalanmaz.
- Frontend'de fiyat, plan, limit, dil, sağlayıcı, platform, feature flag, marketing copy, onboarding step ve SDK download URL hard-coded olmaz.
- Runtime UI/config kuralları için `docs/frontend-config.md` takip edilir.

## Auth
- 2FA email (yayıncı + admin)
- JWT + refresh token

## Not
Fiyatlandırma ve plan limitleri kod'a gömülmez — admin panelden değiştirilir.
Bkz. CLAUDE.md > Konfigürasyon kuralları.
