//! Country → languages / timezone helpers for geo auto-match (file 00 §7 step 5).
//! Timezone normally comes from the proxy check's exit lookup; the table's zone is only a fallback
//! and is given only for single-zone countries (multi-zone countries return None).

/// (ISO-3166 alpha-2, languages in Accept-Language order, single timezone if the country has one)
pub const COUNTRIES: &[(&str, &[&str], Option<&str>)] = &[
    ("AE", &["ar-AE", "ar", "en-US", "en"], Some("Asia/Dubai")),
    ("AR", &["es-AR", "es", "en-US", "en"], None),
    ("AT", &["de-AT", "de", "en-US", "en"], Some("Europe/Vienna")),
    ("AU", &["en-AU", "en"], None),
    ("BD", &["bn-BD", "bn", "en-US", "en"], Some("Asia/Dhaka")),
    ("BE", &["nl-BE", "nl", "fr-BE", "fr", "en-US", "en"], Some("Europe/Brussels")),
    ("BG", &["bg-BG", "bg", "en-US", "en"], Some("Europe/Sofia")),
    ("BR", &["pt-BR", "pt", "en-US", "en"], None),
    ("CA", &["en-CA", "en", "fr-CA", "fr"], None),
    ("CH", &["de-CH", "de", "en-US", "en"], Some("Europe/Zurich")),
    ("CL", &["es-CL", "es", "en-US", "en"], None),
    ("CN", &["zh-CN", "zh", "en-US", "en"], Some("Asia/Shanghai")),
    ("CO", &["es-CO", "es", "en-US", "en"], Some("America/Bogota")),
    ("CZ", &["cs-CZ", "cs", "en-US", "en"], Some("Europe/Prague")),
    ("DE", &["de-DE", "de", "en-US", "en"], Some("Europe/Berlin")),
    ("DK", &["da-DK", "da", "en-US", "en"], Some("Europe/Copenhagen")),
    ("EG", &["ar-EG", "ar", "en-US", "en"], Some("Africa/Cairo")),
    ("ES", &["es-ES", "es", "en-US", "en"], None),
    ("FI", &["fi-FI", "fi", "en-US", "en"], Some("Europe/Helsinki")),
    ("FR", &["fr-FR", "fr", "en-US", "en"], Some("Europe/Paris")),
    ("GB", &["en-GB", "en"], Some("Europe/London")),
    ("GR", &["el-GR", "el", "en-US", "en"], Some("Europe/Athens")),
    ("HK", &["zh-HK", "zh", "en-US", "en"], Some("Asia/Hong_Kong")),
    ("HU", &["hu-HU", "hu", "en-US", "en"], Some("Europe/Budapest")),
    ("ID", &["id-ID", "id", "en-US", "en"], None),
    ("IE", &["en-IE", "en"], Some("Europe/Dublin")),
    ("IL", &["he-IL", "he", "en-US", "en"], Some("Asia/Jerusalem")),
    ("IN", &["en-IN", "en", "hi-IN", "hi"], Some("Asia/Kolkata")),
    ("IT", &["it-IT", "it", "en-US", "en"], Some("Europe/Rome")),
    ("JP", &["ja-JP", "ja", "en-US", "en"], Some("Asia/Tokyo")),
    ("KE", &["en-KE", "en", "sw-KE", "sw"], Some("Africa/Nairobi")),
    ("KR", &["ko-KR", "ko", "en-US", "en"], Some("Asia/Seoul")),
    ("LK", &["si-LK", "si", "en-US", "en"], Some("Asia/Colombo")),
    ("MX", &["es-MX", "es", "en-US", "en"], None),
    ("MY", &["ms-MY", "ms", "en-US", "en"], None),
    ("NG", &["en-NG", "en"], Some("Africa/Lagos")),
    ("NL", &["nl-NL", "nl", "en-US", "en"], Some("Europe/Amsterdam")),
    ("NO", &["nb-NO", "nb", "no", "en-US", "en"], Some("Europe/Oslo")),
    ("NP", &["ne-NP", "ne", "en-US", "en"], Some("Asia/Kathmandu")),
    ("NZ", &["en-NZ", "en"], None),
    ("PE", &["es-PE", "es", "en-US", "en"], Some("America/Lima")),
    ("PH", &["en-PH", "en", "fil-PH", "fil"], Some("Asia/Manila")),
    ("PK", &["en-PK", "en", "ur-PK", "ur"], Some("Asia/Karachi")),
    ("PL", &["pl-PL", "pl", "en-US", "en"], Some("Europe/Warsaw")),
    ("PT", &["pt-PT", "pt", "en-US", "en"], None),
    ("RO", &["ro-RO", "ro", "en-US", "en"], Some("Europe/Bucharest")),
    ("RS", &["sr-RS", "sr", "en-US", "en"], Some("Europe/Belgrade")),
    ("RU", &["ru-RU", "ru", "en-US", "en"], None),
    ("SA", &["ar-SA", "ar", "en-US", "en"], Some("Asia/Riyadh")),
    ("SE", &["sv-SE", "sv", "en-US", "en"], Some("Europe/Stockholm")),
    ("SG", &["en-SG", "en", "zh-SG", "zh"], Some("Asia/Singapore")),
    ("TH", &["th-TH", "th", "en-US", "en"], Some("Asia/Bangkok")),
    ("TR", &["tr-TR", "tr", "en-US", "en"], Some("Europe/Istanbul")),
    ("TW", &["zh-TW", "zh", "en-US", "en"], Some("Asia/Taipei")),
    ("UA", &["uk-UA", "uk", "en-US", "en"], None),
    ("US", &["en-US", "en"], None),
    ("VN", &["vi-VN", "vi", "en-US", "en"], Some("Asia/Ho_Chi_Minh")),
    ("ZA", &["en-ZA", "en"], Some("Africa/Johannesburg")),
];

pub fn country(cc: &str) -> Option<&'static (&'static str, &'static [&'static str], Option<&'static str>)> {
    let cc = cc.to_ascii_uppercase();
    COUNTRIES.iter().find(|(c, _, _)| *c == cc)
}

pub fn languages_for(cc: &str) -> Vec<String> {
    country(cc).map(|(_, l, _)| l.iter().map(|s| s.to_string()).collect()).unwrap_or_else(|| vec!["en-US".into(), "en".into()])
}

pub fn single_timezone(cc: &str) -> Option<&'static str> { country(cc).and_then(|(_, _, tz)| *tz) }

/// Chrome's Accept-Language format: first entry q=1 implied, then q decreasing by 0.1 (min 0.1).
pub fn accept_language(langs: &[String]) -> String {
    langs.iter().enumerate().map(|(i, l)| {
        if i == 0 { l.clone() } else {
            let q = (10i32 - i as i32).max(1);
            format!("{l};q=0.{q}")
        }
    }).collect::<Vec<_>>().join(",")
}

/// Light IANA id syntax check (Area/Location[/Sub]); the authoritative list is checked by the engine's ICU.
pub fn plausible_iana(tz: &str) -> bool {
    if tz == "UTC" { return true; }
    let parts: Vec<&str> = tz.split('/').collect();
    let areas = ["Africa", "America", "Antarctica", "Asia", "Atlantic", "Australia", "Europe", "Indian", "Pacific", "Etc"];
    parts.len() >= 2 && areas.contains(&parts[0])
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "_-+".contains(c)))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn al() {
        assert_eq!(accept_language(&["de-DE".into(), "de".into(), "en-US".into(), "en".into()]), "de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7");
        assert!(plausible_iana("Asia/Dhaka") && plausible_iana("America/Argentina/Buenos_Aires") && !plausible_iana("Mars/Base"));
        assert_eq!(single_timezone("bd"), Some("Asia/Dhaka"));
        assert_eq!(single_timezone("US"), None);
    }
}
