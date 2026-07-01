# Multistream Kararları

## Faz 1 — Mod A (kendi SDK relay'i, birincil)
SDK içine Rust RTMP relay motoru.
Yayıncının kendi internetinden N platforma dağıtım.
Upload gereksinimi: ≥25 Mbps (çoklu platform için yayıncıya uyarı göster).
Senin maliyetin: sıfır (video buluta çıkmaz).

## Faz 1 — Mod A' (yayıncının mevcut servisi, opsiyonel)
Yayıncı isterse kendi Streamlabs hesabını OAuth ile bağlar.
SDK ses kanalını işler, video yayıncının internetinden gider.
Senin maliyetin: sıfır (video buluta çıkmaz).

## Faz 2 — İkili Yayın (SDK relay'e klonlanmış ses eklenir)
İkili yayın: TR ses → hesap 1, EN klonlanmış → hesap 2.
SDK relay motoru Faz 1.6'da kuruldu; burada ses klonlama entegre edilir.

## Faz 3 — Mod C (bulut multistream, premium)
Sadece interneti zayıf yayıncılar için.
Hetzner 1Gbps dedicated → MediaMTX.
Bu yayıncılardan premium ücret alınır.

## İkili Yayın Ban Riski
Twitch/Kick: aynı IP'den iki hesap ban edebilir.
SDK farklı bağlantı/profil yönetmeli (Faz 2'de çöz).

## Entegrasyon Öncelikleri (Faz 1)
1. SDK gömülü RTMP relay (birincil)
2. Streamlabs OAuth (opsiyonel alternatif)
