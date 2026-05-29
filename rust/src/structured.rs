//! C2PA Structured Text embedding (C2PA Technical Specification 2.4, Appendix A.9).
//!
//! Associates a C2PA Manifest Store with a *structured* text asset — source
//! code, configuration files (YAML, TOML, INI), markup (Markdown, AsciiDoc,
//! LaTeX), XML, and similar formats that support a comment or front-matter
//! convention — using an ASCII Armour-style block (modelled on OpenPGP ASCII
//! Armor, RFC 4880 §6.2) delimited by:
//!
//! ```text
//! -----BEGIN C2PA MANIFEST----- <manifest-reference> -----END C2PA MANIFEST-----
//! ```
//!
//! The `<manifest-reference>` is either:
//! - a URL to an external C2PA Manifest Store (preferred), or
//! - a `data:application/c2pa;base64,...` URI embedding the store inline.
//!
//! This is a separate pipeline from the unstructured (Unicode Variation
//! Selector) method in the crate root (Appendix A.8). Neither pipeline is
//! restricted to a fixed set of media types: the implementer chooses which
//! method to use for a given asset. [`recommended_method`] offers an advisory
//! mapping for the media types named in the spec, but it is informative only.
//!
//! Note: `text/html` (Appendix A.7) and `image/svg+xml` (Appendix A.3.3) have
//! their own dedicated embedding methods and are intentionally out of scope for
//! the structured-text pipeline.

use base64::{engine::general_purpose::STANDARD, Engine as _};

/// Fixed opening delimiter (ASCII Armour style, spec A.9.3).
pub const BEGIN_DELIMITER: &str = "-----BEGIN C2PA MANIFEST-----";
/// Fixed closing delimiter (ASCII Armour style, spec A.9.3).
pub const END_DELIMITER: &str = "-----END C2PA MANIFEST-----";
/// Prefix of a `data:` URI carrying a Base64-encoded C2PA Manifest Store.
pub const DATA_URI_PREFIX: &str = "data:application/c2pa;base64,";

/// Where to place the manifest block relative to the host text (spec A.9.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Beginning of the file (preferred by the spec).
    Start,
    /// End of the file — used when the first line is reserved by the host
    /// format (e.g. a `#!` shebang or an `<?xml ?>` declaration), so the
    /// `-----END C2PA MANIFEST-----` delimiter appears on the last line.
    End,
}

/// Advisory recommendation of which embedding method best fits a media type,
/// per the C2PA 2.4 spec text families. Informative only — the implementer may
/// choose either pipeline for any UTF-8 text asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Unicode Variation Selector wrapper (Appendix A.8, this crate's root).
    Unstructured,
    /// ASCII Armour comment/front-matter block (Appendix A.9, this module).
    Structured,
    /// HTML `<script>`/`<link>` method (Appendix A.7) — not implemented here.
    Html,
    /// SVG metadata method (Appendix A.3.3) — not implemented here.
    Svg,
}

/// Result of embedding a structured-text manifest block.
#[derive(Debug, Clone)]
pub struct StructuredEmbed {
    /// The host text with the manifest block inserted.
    pub text: String,
    /// Byte offset of the `c2pa.hash.data` exclusion range (spec A.9.4).
    pub exclusion_start: usize,
    /// Byte length of the `c2pa.hash.data` exclusion range (spec A.9.4).
    pub exclusion_length: usize,
}

/// Result of extracting a structured-text manifest block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredExtraction {
    /// The manifest reference found between the delimiters (URL or `data:` URI),
    /// with surrounding whitespace trimmed.
    pub reference: String,
    /// Decoded Manifest Store bytes — present only when `reference` is a
    /// `data:application/c2pa;base64,...` URI with a valid Base64 payload.
    pub manifest: Option<Vec<u8>>,
}

/// Errors from structured-text extraction. Each variant maps to a normative
/// C2PA validation failure status code (spec A.9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredError {
    /// No block, or only one delimiter — `manifest.structuredText.noManifest`.
    NoManifest,
    /// More than one block — `manifest.structuredText.multipleReferences`.
    MultipleReferences,
    /// Delimiters present but reference empty/whitespace —
    /// `manifest.structuredText.emptyReference`.
    EmptyReference,
}

impl StructuredError {
    /// The C2PA validation status code string for this error.
    pub fn code(self) -> &'static str {
        match self {
            StructuredError::NoManifest => "manifest.structuredText.noManifest",
            StructuredError::MultipleReferences => "manifest.structuredText.multipleReferences",
            StructuredError::EmptyReference => "manifest.structuredText.emptyReference",
        }
    }
}

impl std::fmt::Display for StructuredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl std::error::Error for StructuredError {}

/// Build a `data:application/c2pa;base64,...` URI for a Manifest Store (spec
/// A.9.3.1), using standard Base64 (RFC 4648 §4, with padding, no line breaks).
pub fn encode_data_uri(manifest_bytes: &[u8]) -> String {
    let mut s = String::with_capacity(DATA_URI_PREFIX.len() + (manifest_bytes.len() / 3 + 1) * 4);
    s.push_str(DATA_URI_PREFIX);
    s.push_str(&STANDARD.encode(manifest_bytes));
    s
}

/// Decode a `data:application/c2pa;base64,...` reference into Manifest Store
/// bytes. Returns `None` if `reference` is not such a `data:` URI or the Base64
/// payload is invalid.
pub fn decode_data_uri(reference: &str) -> Option<Vec<u8>> {
    let payload = reference.strip_prefix(DATA_URI_PREFIX)?;
    STANDARD.decode(payload.trim()).ok()
}

/// Build a single-line manifest block (spec A.9.3.1):
///
/// ```text
/// <comment_prefix> -----BEGIN C2PA MANIFEST----- <reference> -----END C2PA MANIFEST----- <comment_suffix>
/// ```
///
/// `comment_suffix` is appended (space-separated) only when non-empty, for
/// block-comment formats such as CSS (`/* */`) or XML/Markdown (`<!-- -->`).
pub fn build_manifest_block(reference: &str, comment_prefix: &str, comment_suffix: &str) -> String {
    let mut block = format!("{comment_prefix} {BEGIN_DELIMITER} {reference} {END_DELIMITER}");
    if !comment_suffix.is_empty() {
        block.push(' ');
        block.push_str(comment_suffix);
    }
    block
}

/// Build a multi-line manifest block for placement inside host front matter
/// (spec A.9.3.2):
///
/// ```text
/// -----BEGIN C2PA MANIFEST-----
/// <reference>
/// -----END C2PA MANIFEST-----
/// ```
///
/// The host front-matter fences (e.g. `---` for YAML) are part of the host
/// format, not the C2PA block, and must be supplied by the caller.
pub fn build_manifest_block_multiline(reference: &str, newline: &str) -> String {
    format!("{BEGIN_DELIMITER}{newline}{reference}{newline}{END_DELIMITER}")
}

/// Embed a manifest block into structured text using the single-line comment
/// form (spec A.9.3.1) and return the resulting text together with the
/// `c2pa.hash.data` exclusion range to bind it (spec A.9.4).
///
/// `newline` is the host file's line terminator — `"\n"` (LF) or `"\r\n"`
/// (CRLF); bare CR is not supported by the spec.
pub fn embed_structured(
    text: &str,
    reference: &str,
    comment_prefix: &str,
    comment_suffix: &str,
    placement: Placement,
    newline: &str,
) -> StructuredEmbed {
    let block = build_manifest_block(reference, comment_prefix, comment_suffix);
    match placement {
        Placement::Start => {
            // block + newline + text. Exclusion = block plus its trailing terminator.
            let mut out = String::with_capacity(block.len() + newline.len() + text.len());
            out.push_str(&block);
            out.push_str(newline);
            out.push_str(text);
            StructuredEmbed {
                text: out,
                exclusion_start: 0,
                exclusion_length: block.len() + newline.len(),
            }
        }
        Placement::End => {
            // text + newline + block. Exclusion starts at the preceding newline.
            let mut out = String::with_capacity(text.len() + newline.len() + block.len());
            out.push_str(text);
            let start = out.len();
            out.push_str(newline);
            out.push_str(&block);
            StructuredEmbed {
                text: out,
                exclusion_start: start,
                exclusion_length: newline.len() + block.len(),
            }
        }
    }
}

/// Extract a manifest reference from structured text (spec A.9.5). Form-agnostic:
/// the reference is whatever appears between the single pair of delimiters,
/// trimmed of surrounding whitespace, so both the single-line and front-matter
/// forms are handled.
pub fn extract_structured(text: &str) -> Result<StructuredExtraction, StructuredError> {
    let begin_count = text.matches(BEGIN_DELIMITER).count();
    let end_count = text.matches(END_DELIMITER).count();
    if begin_count == 0 || end_count == 0 {
        return Err(StructuredError::NoManifest);
    }
    if begin_count > 1 || end_count > 1 {
        return Err(StructuredError::MultipleReferences);
    }
    // Safe: counts are exactly 1, so both delimiters are present.
    let begin = text.find(BEGIN_DELIMITER).unwrap() + BEGIN_DELIMITER.len();
    let end = text.find(END_DELIMITER).unwrap();
    if end <= begin {
        return Err(StructuredError::NoManifest);
    }
    let reference = text[begin..end].trim();
    if reference.is_empty() {
        return Err(StructuredError::EmptyReference);
    }
    Ok(StructuredExtraction {
        reference: reference.to_string(),
        manifest: decode_data_uri(reference),
    })
}

/// Advisory recommendation of an embedding method for a media type, per the
/// C2PA 2.4 spec text families. Returns `None` for media types with no defined
/// text embedding method. Informative only — see module docs.
pub fn recommended_method(mime: &str) -> Option<Method> {
    match mime {
        // Unstructured family (A.8). JSON and CSV have no comment/front-matter
        // syntax, so the structured method (A.9) is not applicable to them; the
        // variation-selector method is the only embedded option.
        "text/plain" | "text/markdown" | "text/csv" | "application/json" => {
            Some(Method::Unstructured)
        }
        // Structured family (A.9), via XML comment syntax `<!-- -->`.
        "text/xml" | "application/xml" | "application/xhtml+xml" => Some(Method::Structured),
        // Dedicated methods not implemented by this crate.
        "text/html" => Some(Method::Html),
        "image/svg+xml" => Some(Method::Svg),
        // Any other text/* with a comment convention defaults to structured.
        other if other.starts_with("text/") => Some(Method::Structured),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_uri_round_trip() {
        let bytes = [0x01u8, 0x02, 0x03, 0xff, 0x00];
        let uri = encode_data_uri(&bytes);
        assert_eq!(uri, "data:application/c2pa;base64,AQID/wA=");
        assert_eq!(decode_data_uri(&uri), Some(bytes.to_vec()));
        assert_eq!(decode_data_uri("https://example.com/m.c2pa"), None);
        assert_eq!(decode_data_uri("data:application/c2pa;base64,!!!!"), None);
    }

    #[test]
    fn single_line_block_format() {
        // Python-style line comment, no suffix.
        assert_eq!(
            build_manifest_block("https://x/m.c2pa", "#", ""),
            "# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----"
        );
        // XML/Markdown block comment, with suffix.
        assert_eq!(
            build_manifest_block("https://x/m.c2pa", "<!--", "-->"),
            "<!-- -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST----- -->"
        );
    }

    #[test]
    fn multiline_block_format() {
        assert_eq!(
            build_manifest_block_multiline("https://x/m.c2pa", "\n"),
            "-----BEGIN C2PA MANIFEST-----\nhttps://x/m.c2pa\n-----END C2PA MANIFEST-----"
        );
    }

    #[test]
    fn embed_at_start_exclusion_and_round_trip() {
        let r = embed_structured(
            "body line 1\nbody line 2\n",
            "https://x/m.c2pa",
            "#",
            "",
            Placement::Start,
            "\n",
        );
        let block = "# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----";
        assert_eq!(r.text, format!("{block}\nbody line 1\nbody line 2\n"));
        assert_eq!(r.exclusion_start, 0);
        assert_eq!(r.exclusion_length, block.len() + 1); // + LF
                                                         // Exclusion range covers exactly the block + its trailing terminator.
        assert_eq!(
            &r.text.as_bytes()[r.exclusion_start..r.exclusion_start + r.exclusion_length],
            format!("{block}\n").as_bytes()
        );
        let x = extract_structured(&r.text).unwrap();
        assert_eq!(x.reference, "https://x/m.c2pa");
        assert_eq!(x.manifest, None);
    }

    #[test]
    fn embed_at_end_exclusion_starts_at_preceding_newline() {
        let text = "#!/usr/bin/env python\nprint('hi')\n";
        let r = embed_structured(text, "https://x/m.c2pa", "#", "", Placement::End, "\n");
        let block = "# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----";
        assert_eq!(r.text, format!("{text}\n{block}"));
        assert_eq!(r.exclusion_start, text.len());
        assert_eq!(r.exclusion_length, 1 + block.len()); // preceding LF + block
        assert_eq!(
            &r.text.as_bytes()[r.exclusion_start..r.exclusion_start + r.exclusion_length],
            format!("\n{block}").as_bytes()
        );
    }

    #[test]
    fn embed_and_extract_data_uri() {
        let manifest = b"\xde\xad\xbe\xef";
        let uri = encode_data_uri(manifest);
        let r = embed_structured("doc\n", &uri, "//", "", Placement::Start, "\n");
        let x = extract_structured(&r.text).unwrap();
        assert_eq!(x.reference, uri);
        assert_eq!(x.manifest.as_deref(), Some(&manifest[..]));
    }

    #[test]
    fn extract_errors() {
        assert_eq!(
            extract_structured("no manifest here"),
            Err(StructuredError::NoManifest)
        );
        // Only one delimiter.
        assert_eq!(
            extract_structured("# -----BEGIN C2PA MANIFEST----- https://x"),
            Err(StructuredError::NoManifest)
        );
        // Empty reference.
        assert_eq!(
            extract_structured("# -----BEGIN C2PA MANIFEST-----   -----END C2PA MANIFEST-----"),
            Err(StructuredError::EmptyReference)
        );
        // Multiple blocks.
        let two = "# -----BEGIN C2PA MANIFEST----- a -----END C2PA MANIFEST-----\n# -----BEGIN C2PA MANIFEST----- b -----END C2PA MANIFEST-----";
        assert_eq!(
            extract_structured(two),
            Err(StructuredError::MultipleReferences)
        );
    }

    #[test]
    fn front_matter_form_extracts() {
        let doc = "---\n-----BEGIN C2PA MANIFEST-----\nhttps://x/m.c2pa\n-----END C2PA MANIFEST-----\ntitle: Doc\n---\nbody\n";
        let x = extract_structured(doc).unwrap();
        assert_eq!(x.reference, "https://x/m.c2pa");
    }

    #[test]
    fn recommended_methods() {
        assert_eq!(recommended_method("text/plain"), Some(Method::Unstructured));
        assert_eq!(
            recommended_method("application/json"),
            Some(Method::Unstructured)
        );
        assert_eq!(recommended_method("text/csv"), Some(Method::Unstructured));
        assert_eq!(
            recommended_method("application/xml"),
            Some(Method::Structured)
        );
        assert_eq!(
            recommended_method("text/x-python"),
            Some(Method::Structured)
        );
        assert_eq!(recommended_method("text/html"), Some(Method::Html));
        assert_eq!(recommended_method("image/svg+xml"), Some(Method::Svg));
        assert_eq!(recommended_method("image/jpeg"), None);
    }
}
