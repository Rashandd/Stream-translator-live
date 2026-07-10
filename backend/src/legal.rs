//! Legal document HTML boundary.
//!
//! Legal copy is authored by an administrator but rendered on public pages, so
//! it is treated as untrusted stored input.  The allowlist deliberately stays
//! small: legal documents need headings, paragraphs, lists, emphasis, quotes,
//! and links—not executable or embedded content.

use std::collections::{HashMap, HashSet};

use ammonia::Builder;
use sha2::{Digest, Sha256};

/// Runtime config keys whose values contain admin-authored legal HTML.
pub const HTML_CONFIG_KEYS: [&str; 6] = [
    "legal.terms.html",
    "legal.privacy.html",
    "legal.kvkk.html",
    "legal.cookies.html",
    "legal.dpa.html",
    "legal.communications.html",
];

/// Whether a service-config key is a public legal HTML document.
pub fn is_html_config_key(key: &str) -> bool {
    HTML_CONFIG_KEYS.contains(&key)
}

/// Clean legal HTML with an explicit allowlist.
///
/// Disallowed active-content elements (including their content), event
/// attributes, SVG, embedded documents, and unsafe URL schemes are removed by
/// a parser-backed sanitizer. Regex-based HTML rewriting is intentionally not
/// used.
pub fn sanitize_html(input: &str) -> String {
    let tags = HashSet::from([
        "h1",
        "h2",
        "h3",
        "p",
        "ul",
        "ol",
        "li",
        "strong",
        "em",
        "a",
        "blockquote",
    ]);
    let tag_attributes = HashMap::from([("a", HashSet::from(["href", "title"]))]);
    let url_schemes = HashSet::from(["http", "https", "mailto"]);

    Builder::default()
        .tags(tags)
        .generic_attributes(HashSet::new())
        .tag_attributes(tag_attributes)
        .url_schemes(url_schemes)
        .link_rel(Some("noopener noreferrer"))
        .clean(input)
        .to_string()
}

/// Apply the legal HTML boundary only to matching service-config values.
pub fn sanitize_config_value(key: &str, value: &str) -> String {
    if is_html_config_key(key) {
        sanitize_html(value)
    } else {
        value.to_string()
    }
}

/// Stable, non-reversible summary for legal-config audit records.
///
/// Raw legal payloads must not be copied into logs. Callers can record this
/// digest with the config key and authenticated actor metadata instead.
pub fn audit_digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_keeps_document_structure_and_safe_links() {
        let input = concat!(
            "<h1>Terms</h1><h2>Use</h2><h3>Details</h3>",
            "<p>Read <strong>carefully</strong> and <em>ask</em>.</p>",
            "<ul><li>One</li></ul><ol><li>Two</li></ol>",
            "<blockquote>Plain language</blockquote>",
            "<a href=\"https://strovak.com/legal\" title=\"Legal\">Details</a>",
        );

        let clean = sanitize_html(input);
        for expected in [
            "<h1>Terms</h1>",
            "<strong>carefully</strong>",
            "<em>ask</em>",
            "<ul><li>One</li></ul>",
            "<blockquote>Plain language</blockquote>",
            "https://strovak.com/legal",
        ] {
            assert!(clean.contains(expected), "clean HTML: {clean}");
        }
    }

    #[test]
    fn executable_payloads_are_removed() {
        let input = concat!(
            "<script>alert(1)</script><style>body{display:none}</style>",
            "<iframe srcdoc=\"<script>alert(2)</script>\"></iframe>",
            "<object data=x></object><embed src=x>",
            "<svg><a xlink:href=\"javascript:alert(3)\">svg</a></svg>",
            "<p onclick=alert(4)>Safe text</p>",
            "<a href=\"javascript:alert(5)\">unsafe link</a>",
            "<a href=\"jav&#x61;script:alert(6)\">encoded link</a>",
            "<a href=\"data:text/html,test\">data link</a>",
            "<scr<script>ipt>alert(7)</scr</script>ipt><p>malformed</p>",
        );

        let clean = sanitize_html(input).to_ascii_lowercase();
        for forbidden in [
            "<script",
            "alert(1)",
            "<style",
            "display:none",
            "<iframe",
            "<object",
            "<embed",
            "<svg",
            "xlink:",
            "onclick",
            "javascript:",
            "data:text/html",
        ] {
            assert!(!clean.contains(forbidden), "clean HTML: {clean}");
        }
    }

    #[test]
    fn audit_digest_is_stable_without_exposing_the_payload() {
        let digest = audit_digest("<h1>Private draft text</h1>");
        assert_eq!(digest.len(), 64);
        assert_eq!(digest, audit_digest("<h1>Private draft text</h1>"));
        assert!(!digest.contains("Private draft text"));
    }
}
