//! C2PA HTML embedding (C2PA Technical Specification 2.4, Appendix A.7).
//!
//! Associates a C2PA Manifest Store with an HTML document using one of the two
//! methods the spec defines, both keyed on the IANA media type `application/c2pa`:
//!
//! - **Inline**: a `<script type="application/c2pa">` element in the `<head>`
//!   whose content is the Base64-encoded Manifest Store.
//! - **Referenced** (preferred): a `<link rel="c2pa-manifest" href="...">`
//!   element in the `<head>` pointing at an external Manifest Store.
//!
//! A document shall carry at most one association; more than one is the
//! `manifest.html.multipleManifests` validation failure (spec A.7.1).
//!
//! Separate pipeline from the unstructured (A.8) and structured (A.9) methods.
//! Wire-compatible (byte-identical output) with the Python, TypeScript and Go
//! modules for the same inputs.

use base64::{engine::general_purpose::STANDARD, Engine as _};

/// IANA media type for a C2PA Manifest Store.
pub const C2PA_MEDIA_TYPE: &str = "application/c2pa";
const SCRIPT_OPEN: &str = "<script type=\"application/c2pa\">";
const SCRIPT_CLOSE: &str = "</script>";
const HEAD_CLOSE: &str = "</head>";

/// C2PA validation status code for more than one manifest association (A.7.1).
pub const MULTIPLE_MANIFESTS: &str = "manifest.html.multipleManifests";
/// Embed-time error code: the host document has no `</head>`.
pub const NO_HEAD: &str = "html.noHead";

/// HTML embedding/extraction error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlError {
    /// More than one manifest association — `manifest.html.multipleManifests`.
    MultipleManifests,
    /// No `</head>` to place the manifest element — `html.noHead`.
    NoHead,
}

impl HtmlError {
    /// The status / error code string.
    pub fn code(self) -> &'static str {
        match self {
            HtmlError::MultipleManifests => MULTIPLE_MANIFESTS,
            HtmlError::NoHead => NO_HEAD,
        }
    }
}

impl std::fmt::Display for HtmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl std::error::Error for HtmlError {}

/// Inline embedding method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlMethod {
    /// `<script type="application/c2pa">` element with an inline manifest.
    Inline,
    /// `<link rel="c2pa-manifest">` element referencing an external manifest.
    Reference,
}

/// Result of embedding an inline manifest into HTML.
#[derive(Debug, Clone)]
pub struct HtmlEmbed {
    /// The document with the `<script>` manifest element inserted.
    pub html: String,
    /// Byte offset of the `c2pa.hash.data` exclusion range (spec A.7.1.3).
    pub exclusion_start: usize,
    /// Byte length of the exclusion range (the entire `<script>` element).
    pub exclusion_length: usize,
}

/// Result of extracting a manifest association from HTML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlExtraction {
    /// Which method was found.
    pub method: HtmlMethod,
    /// Decoded Manifest Store bytes, present only for [`HtmlMethod::Inline`].
    pub manifest: Option<Vec<u8>>,
    /// External manifest URL, present only for [`HtmlMethod::Reference`].
    pub reference: Option<String>,
}

/// Build a `<script type="application/c2pa">...</script>` element (A.7.1.1).
pub fn build_html_script(manifest_bytes: &[u8]) -> String {
    format!(
        "{SCRIPT_OPEN}{}{SCRIPT_CLOSE}",
        STANDARD.encode(manifest_bytes)
    )
}

/// Build a `<link rel="c2pa-manifest" href="..." type="application/c2pa">` (A.7.1.2).
pub fn build_html_link(url: &str) -> String {
    format!("<link rel=\"c2pa-manifest\" href=\"{url}\" type=\"application/c2pa\">")
}

/// Embed a Manifest Store inline as a `<script>` element placed just before
/// `</head>`, returning the document and the `c2pa.hash.data` exclusion range
/// covering the element (spec A.7.1.1, A.7.1.3).
pub fn embed_html_inline(
    html: &str,
    manifest_bytes: &[u8],
    newline: &str,
) -> Result<HtmlEmbed, HtmlError> {
    let element = build_html_script(manifest_bytes);
    let idx = html.find(HEAD_CLOSE).ok_or(HtmlError::NoHead)?;
    let mut out = String::with_capacity(html.len() + element.len() + newline.len());
    out.push_str(&html[..idx]);
    out.push_str(&element);
    out.push_str(newline);
    out.push_str(&html[idx..]);
    Ok(HtmlEmbed {
        html: out,
        exclusion_start: idx,
        exclusion_length: element.len(),
    })
}

/// Embed a reference to an external Manifest Store as a `<link>` element placed
/// just before `</head>` (spec A.7.1.2). The referenced method's hard binding
/// has no exclusion range (the hash covers the whole document).
pub fn embed_html_reference(html: &str, url: &str, newline: &str) -> Result<String, HtmlError> {
    let element = build_html_link(url);
    let idx = html.find(HEAD_CLOSE).ok_or(HtmlError::NoHead)?;
    let mut out = String::with_capacity(html.len() + element.len() + newline.len());
    out.push_str(&html[..idx]);
    out.push_str(&element);
    out.push_str(newline);
    out.push_str(&html[idx..]);
    Ok(out)
}

fn find_script_contents(html: &str) -> Vec<&str> {
    let mut results = Vec::new();
    let mut pos = 0;
    while let Some(rel_i) = html[pos..].find("<script") {
        let i = pos + rel_i;
        let Some(rel_gt) = html[i..].find('>') else {
            break;
        };
        let gt = i + rel_gt;
        let tag = &html[i..=gt];
        if tag.contains("type=\"application/c2pa\"") {
            if let Some(rel_end) = html[gt + 1..].find(SCRIPT_CLOSE) {
                let end = gt + 1 + rel_end;
                results.push(&html[gt + 1..end]);
                pos = end + SCRIPT_CLOSE.len();
                continue;
            }
        }
        pos = gt + 1;
    }
    results
}

fn find_link_tags(html: &str) -> Vec<&str> {
    let mut results = Vec::new();
    let mut pos = 0;
    while let Some(rel_i) = html[pos..].find("<link") {
        let i = pos + rel_i;
        let Some(rel_gt) = html[i..].find('>') else {
            break;
        };
        let gt = i + rel_gt;
        let tag = &html[i..=gt];
        if tag.contains("rel=\"c2pa-manifest\"") {
            results.push(tag);
        }
        pos = gt + 1;
    }
    results
}

fn href_of(tag: &str) -> Option<String> {
    let marker = "href=\"";
    let i = tag.find(marker)?;
    let start = i + marker.len();
    let end = tag[start..].find('"')?;
    Some(tag[start..start + end].to_string())
}

/// Extract a manifest association from an HTML document (spec A.7.1.4). Returns
/// `Ok(None)` if no association is present, and `Err(MultipleManifests)` if more
/// than one association is found.
pub fn extract_html(html: &str) -> Result<Option<HtmlExtraction>, HtmlError> {
    let scripts = find_script_contents(html);
    let links = find_link_tags(html);
    let total = scripts.len() + links.len();
    if total == 0 {
        return Ok(None);
    }
    if total > 1 {
        return Err(HtmlError::MultipleManifests);
    }
    if let Some(content) = scripts.first() {
        let manifest = STANDARD.decode(content.trim()).ok();
        return Ok(Some(HtmlExtraction {
            method: HtmlMethod::Inline,
            manifest,
            reference: None,
        }));
    }
    Ok(Some(HtmlExtraction {
        method: HtmlMethod::Reference,
        manifest: None,
        reference: href_of(links[0]),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>Example</title>\n</head>\n<body>\n<p>Content here.</p>\n</body>\n</html>\n";

    #[test]
    fn builders() {
        assert_eq!(
            build_html_script(&[0xDE, 0xAD, 0xBE, 0xEF]),
            "<script type=\"application/c2pa\">3q2+7w==</script>"
        );
        assert_eq!(
            build_html_link("https://x/m.c2pa"),
            "<link rel=\"c2pa-manifest\" href=\"https://x/m.c2pa\" type=\"application/c2pa\">"
        );
    }

    #[test]
    fn inline_exclusion_and_round_trip() {
        let manifest = [0xDEu8, 0xAD, 0xBE, 0xEF];
        let r = embed_html_inline(HTML, &manifest, "\n").unwrap();
        let element = "<script type=\"application/c2pa\">3q2+7w==</script>";
        assert_eq!(
            &r.html.as_bytes()[r.exclusion_start..r.exclusion_start + r.exclusion_length],
            element.as_bytes()
        );
        assert!(r.html.contains(&format!("{element}\n</head>")));
        let x = extract_html(&r.html).unwrap().unwrap();
        assert_eq!(x.method, HtmlMethod::Inline);
        assert_eq!(x.manifest.as_deref(), Some(&manifest[..]));
    }

    #[test]
    fn inline_no_head() {
        assert_eq!(
            embed_html_inline("<p>no head</p>", &[0], "\n").unwrap_err(),
            HtmlError::NoHead
        );
    }

    #[test]
    fn reference_round_trip() {
        let url = "https://fabrikam.com/manifest.c2pa";
        let html = embed_html_reference(HTML, url, "\n").unwrap();
        let x = extract_html(&html).unwrap().unwrap();
        assert_eq!(x.method, HtmlMethod::Reference);
        assert_eq!(x.reference.as_deref(), Some(url));
        assert_eq!(x.manifest, None);
    }

    #[test]
    fn extract_none() {
        assert_eq!(extract_html(HTML).unwrap(), None);
    }

    #[test]
    fn multiple_manifests() {
        let r = embed_html_inline(HTML, &[0], "\n").unwrap();
        let doubled = embed_html_reference(&r.html, "https://x/m.c2pa", "\n").unwrap();
        assert_eq!(extract_html(&doubled), Err(HtmlError::MultipleManifests));
    }
}
