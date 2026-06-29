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
3. Servis Toggle: STT/Çeviri/TTS provider anlık geçiş (restart yok)
4. Fiyatlandırma Yönetimi: plan fiyatları, limitler, özellikler DB'den
5. Maliyet Özeti: servis bazlı harcama

## Auth
- 2FA email (yayıncı + admin)
- JWT + refresh token

## Not
Fiyatlandırma ve plan limitleri kod'a gömülmez — admin panelden değiştirilir.
Bkz. CLAUDE.md > Konfigürasyon kuralları.
