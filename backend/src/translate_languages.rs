//! DeepL hedef dil kataloğu (Faz 2.0 — roadmap "DeepL Dil Matrisi Genişletme").
//!
//! `gateway::deepl::DeeplTranslate::translate` zaten rastgele bir `target`
//! kabul ediyor (Faz 1.8'den beri hard-coded liste yok) — asıl eksik olan,
//! hangi kodların GERÇEKTEN DeepL tarafından desteklendiğini bilen ve
//! admin/yayıncı panelinde seçilebilir hale getiren bu katalogdu.
//!
//! Kod, `gateway::deepl`'in bugünkü basit-kod davranışını korur: `en`/`pt`
//! burada bölgesiz (düz) kod olarak listelenir, `to_uppercase()` ile olduğu
//! gibi DeepL'e gider. DeepL'in gerçek API'si hedef için `EN-GB`/`EN-US` ve
//! `PT-PT`/`PT-BR` gibi bölgesel varyant isteyebilir (düz `EN`/`PT` hedef
//! olarak reddedilebilir) — bu, canlı bir DeepL key'iyle doğrulanmadı; bu
//! katalog yalnızca Faz 1.8'in mevcut basitleştirmesini genişletiyor,
//! bölgesel varyant desteği ayrı bir iş olarak kalır.

/// Tek bir DeepL hedef dili: kod, İngilizce görünen ad, formality
/// (resmi/samimi üslup) desteği. `supports_formality` DeepL'in genel
/// duyurduğu dil kümesine göre — canlı `/v2/languages` ile periyodik
/// doğrulanması önerilir (liste zamanla değişebilir).
pub struct DeeplLanguage {
    pub code: &'static str,
    pub name: &'static str,
    pub supports_formality: bool,
}

/// Faz 1.8'in TR→EN/ES/DE/FR sınırlamasını kaldıran tam katalog seed'i.
pub const DEEPL_TARGET_LANGUAGES: &[DeeplLanguage] = &[
    lang("en", "English", true),
    lang("de", "German", true),
    lang("fr", "French", true),
    lang("es", "Spanish", true),
    lang("tr", "Turkish", true),
    lang("it", "Italian", true),
    lang("pt", "Portuguese", true),
    lang("nl", "Dutch", true),
    lang("pl", "Polish", true),
    lang("ru", "Russian", true),
    lang("ja", "Japanese", true),
    lang("zh", "Chinese", false),
    lang("ko", "Korean", false),
    lang("sv", "Swedish", false),
    lang("da", "Danish", false),
    lang("fi", "Finnish", false),
    lang("nb", "Norwegian", false),
    lang("cs", "Czech", false),
    lang("sk", "Slovak", false),
    lang("ro", "Romanian", false),
    lang("bg", "Bulgarian", false),
    lang("el", "Greek", false),
    lang("hu", "Hungarian", false),
    lang("uk", "Ukrainian", false),
    lang("id", "Indonesian", false),
    lang("lt", "Lithuanian", false),
    lang("lv", "Latvian", false),
    lang("et", "Estonian", false),
    lang("ar", "Arabic", false),
    lang("sl", "Slovenian", false),
];

const fn lang(code: &'static str, name: &'static str, supports_formality: bool) -> DeeplLanguage {
    DeeplLanguage { code, name, supports_formality }
}

/// Katalogdaki tüm kodların virgülle ayrılmış hâli — config seed'i için.
pub fn all_codes_csv() -> String {
    DEEPL_TARGET_LANGUAGES
        .iter()
        .map(|l| l.code)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_no_duplicate_codes() {
        let mut codes: Vec<&str> = DEEPL_TARGET_LANGUAGES.iter().map(|l| l.code).collect();
        let before = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), before, "katalogda tekrar eden dil kodu var");
    }

    #[test]
    fn catalog_covers_the_phase_one_four_languages() {
        let codes: Vec<&str> = DEEPL_TARGET_LANGUAGES.iter().map(|l| l.code).collect();
        for expected in ["en", "es", "de", "fr"] {
            assert!(codes.contains(&expected), "Faz 1.8 dili katalogdan düştü: {expected}");
        }
    }

    #[test]
    fn all_codes_csv_joins_every_entry() {
        let csv = all_codes_csv();
        assert_eq!(csv.split(',').count(), DEEPL_TARGET_LANGUAGES.len());
        assert!(csv.contains("tr"));
    }
}
