# Multistream Kararları

## Faz 1 — Mod A (yayıncının mevcut servisi)
Yayıncı kendi Restream/Castr hesabını OAuth ile bağlar.
SDK ses kanalını işler, video yayıncının internetinden gider.
Senin maliyetin: sıfır (video buluta çıkmaz).

## Faz 2 — Mod B (SDK gömülü RTMP relay)
SDK içine Rust RTMP relay motoru.
Yayıncının kendi internetinden N platforma dağıtım.
İkili yayın: TR ses → hesap 1, EN klonlanmış → hesap 2.
Upload gereksinimi: ≥25 Mbps (yayıncıya uyarı göster).

## Faz 3 — Mod C (bulut multistream, premium)
Sadece interneti zayıf yayıncılar için.
Hetzner 1Gbps dedicated → MediaMTX.
Bu yayıncılardan premium ücret alınır.

## İkili Yayın Ban Riski
Twitch/Kick: aynı IP'den iki hesap ban edebilir.
SDK farklı bağlantı/profil yönetmeli (Faz 2'de çöz).

## Entegrasyon Öncelikleri (Faz 1)
1. Restream OAuth + API
2. Castr OAuth + API
3. Streamlabs OAuth
