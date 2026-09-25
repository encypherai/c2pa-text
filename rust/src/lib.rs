//! C2PA Text Manifest Wrapper Reference Implementation.
//!
//! This module implements the C2PA Text Embedding Standard, allowing binary data
//! (typically a C2PA JUMBF Manifest) to be embedded into valid UTF-8 strings using
//! invisible Unicode Variation Selectors.
//!
//! # Validation
//!
//! Use [`validate_manifest`] to check manifest structure before embedding.
//! This helps catch issues early and provides detailed diagnostics.

use std::char;
use unicode_normalization::UnicodeNormalization;

pub mod html;
pub mod structured;
pub mod validator;
pub use validator::{
    validate_jumbf_structure, validate_manifest, validate_text, validate_wrapper_bytes,
    ValidationCode, ValidationIssue, ValidationResult,
};

// ---------------------- Constants -------------------------------------------

const MAGIC: &[u8; 8] = b"C2PATXT\0";
const VERSION: u8 = 1;
const HEADER_SIZE: usize = 13; // 8 (Magic) + 1 (Version) + 4 (Length)
const ZWNBSP: char = '\u{feff}';

// Variation Selector Ranges
const VS_START: u32 = 0xFE00;
const VS_END: u32 = 0xFE0F;
const VS_SUP_START: u32 = 0xE0100;
const VS_SUP_END: u32 = 0xE01EF;

#[derive(Debug)]
pub enum Error {
    InvalidByte(u8),
    InvalidVariationSelector(char),
    TooShort,
    InvalidMagic,
    UnsupportedVersion,
    Truncated,
    MultipleWrappers,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidByte(b) => write!(f, "Byte out of range: {}", b),
            Error::InvalidVariationSelector(c) => write!(f, "Invalid variation selector: {}", c),
            Error::TooShort => write!(f, "Sequence too short for header"),
            Error::InvalidMagic => write!(f, "Invalid magic bytes"),
            Error::UnsupportedVersion => write!(f, "Unsupported version"),
            Error::Truncated => write!(f, "Wrapper truncated before end of manifest"),
            Error::MultipleWrappers => write!(f, "Multiple C2PA wrappers detected"),
        }
    }
}

impl std::error::Error for Error {}

fn byte_to_vs(byte: u8) -> char {
    if byte <= 15 {
        char::from_u32(VS_START + byte as u32).unwrap()
    } else {
        char::from_u32(VS_SUP_START + (byte as u32) - 16).unwrap()
    }
}

fn vs_to_byte(c: char) -> Option<u8> {
    let code = c as u32;
    if (VS_START..=VS_END).contains(&code) {
        Some((code - VS_START) as u8)
    } else if (VS_SUP_START..=VS_SUP_END).contains(&code) {
        Some(((code - VS_SUP_START) + 16) as u8)
    } else {
        None
    }
}

/// Encode raw bytes into a C2PA Text Manifest Wrapper string.
pub fn encode_wrapper(manifest_bytes: &[u8]) -> String {
    let len = manifest_bytes.len() as u32;

    // Estimate capacity: 1 (ZWNBSP) + HEADER_SIZE + len
    let mut out = String::with_capacity(1 + HEADER_SIZE + manifest_bytes.len());
    out.push(ZWNBSP);

    // Encode Header
    for &b in MAGIC {
        out.push(byte_to_vs(b));
    }
    out.push(byte_to_vs(VERSION));

    // Length (Big Endian)
    out.push(byte_to_vs(((len >> 24) & 0xFF) as u8));
    out.push(byte_to_vs(((len >> 16) & 0xFF) as u8));
    out.push(byte_to_vs(((len >> 8) & 0xFF) as u8));
    out.push(byte_to_vs((len & 0xFF) as u8));

    // Encode Body
    for &b in manifest_bytes {
        out.push(byte_to_vs(b));
    }

    out
}

/// Compute the deterministic target UTF-8 byte length of a padded wrapper
/// for a manifest of `manifest_byte_count` bytes.
///
/// Formula: `3 + (13 + M) * 4 + 6`
///
/// The margin of 6 guarantees the gap between target and actual is always
/// expressible as `3a + 4b` (required for VS-based padding).
pub fn worst_case_wrapper_byte_length(manifest_byte_count: usize) -> usize {
    3 + (HEADER_SIZE + manifest_byte_count) * 4 + 6
}

/// Compute padding bytes whose VS encoding totals exactly `gap` UTF-8 bytes.
/// Returns a Vec of byte values (0x00 for 3-byte VS, 0x10 for 4-byte VS).
fn compute_padding(gap: usize) -> Result<Vec<u8>, Error> {
    if gap == 0 {
        return Ok(Vec::new());
    }
    // Spec decomposition: b = gap mod 3 makes `gap - 4b` divisible by 3.
    let b = gap % 3;
    let four_b = 4 * b;
    if gap < four_b {
        return Err(Error::Truncated); // 1, 2 and 5 are not expressible as 3a + 4b
    }
    let a = (gap - four_b) / 3;
    let mut result = vec![0x00u8; a];
    result.extend(vec![0x10u8; b]);
    Ok(result)
}

/// Encode a C2PA Text Manifest Wrapper and pad to an exact UTF-8 byte length.
///
/// Decoders use `manifestLength` to extract the manifest and ignore trailing
/// padding bytes.
pub fn encode_wrapper_padded(
    manifest_bytes: &[u8],
    target_byte_length: usize,
) -> Result<String, Error> {
    let base = encode_wrapper(manifest_bytes);
    let actual = base.len(); // String::len() returns UTF-8 byte count
    if target_byte_length < actual {
        return Err(Error::Truncated);
    }
    let gap = target_byte_length - actual;
    if gap == 0 {
        return Ok(base);
    }
    let padding = compute_padding(gap)?;
    let mut result = base;
    for &b in &padding {
        result.push(byte_to_vs(b));
    }
    Ok(result)
}

/// Embed a C2PA manifest into text.
/// Normalizes the text to NFC and appends the invisible wrapper.
pub fn embed_manifest(text: &str, manifest_bytes: &[u8]) -> String {
    let normalized: String = text.nfc().collect();
    let wrapper = encode_wrapper(manifest_bytes);
    format!("{}{}", normalized, wrapper)
}

/// Result of extracting a manifest
#[derive(Debug)]
pub struct ExtractionResult {
    pub manifest: Option<Vec<u8>>,
    pub clean_text: String,
    pub offset: Option<usize>, // Byte offset of the wrapper start
    pub length: Option<usize>, // Byte length of the wrapper
}

/// Extract a C2PA manifest from text.
/// Returns ExtractionResult.
pub fn extract_manifest(text: &str) -> Result<ExtractionResult, Error> {
    // Simple scan for ZWNBSP
    let mut wrapper_start = None;
    let mut wrapper_end = None;
    let mut decoded_bytes = Vec::new();

    // Iterate chars to find potential wrapper
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;

    while i < chars.len() {
        let (idx, c) = chars[i];
        if c == ZWNBSP {
            // Potential start
            let start_idx = idx;
            let mut current_bytes = Vec::new();
            let mut j = i + 1;

            while j < chars.len() {
                let (_, vc) = chars[j];
                if let Some(b) = vs_to_byte(vc) {
                    current_bytes.push(b);
                    j += 1;
                } else {
                    break; // End of sequence
                }
            }

            // Check header if we have enough bytes
            if current_bytes.len() >= HEADER_SIZE {
                // Check Magic
                if &current_bytes[0..8] == MAGIC {
                    // Check Version
                    if current_bytes[8] == VERSION {
                        // Check Length
                        let len = u32::from_be_bytes([
                            current_bytes[9],
                            current_bytes[10],
                            current_bytes[11],
                            current_bytes[12],
                        ]) as usize;

                        if current_bytes.len() >= HEADER_SIZE + len {
                            // Found valid wrapper
                            if wrapper_start.is_some() {
                                return Err(Error::MultipleWrappers);
                            }
                            wrapper_start = Some(start_idx);
                            // Calculate end index in bytes
                            if j < chars.len() {
                                wrapper_end = Some(chars[j].0);
                            } else {
                                wrapper_end = Some(text.len());
                            }

                            decoded_bytes = current_bytes[HEADER_SIZE..HEADER_SIZE + len].to_vec();

                            // We found one, but spec says we must ensure no others exist.
                            // Continue searching from j
                            i = j;
                            continue;
                        }
                    }
                }
            }
        }
        i += 1;
    }

    if let (Some(start), Some(end)) = (wrapper_start, wrapper_end) {
        let pre = &text[..start];
        let post = &text[end..];
        let clean: String = format!("{}{}", pre, post).nfc().collect();
        Ok(ExtractionResult {
            manifest: Some(decoded_bytes),
            clean_text: clean,
            offset: Some(start),
            length: Some(end - start),
        })
    } else {
        Ok(ExtractionResult {
            manifest: None,
            clean_text: text.nfc().collect(),
            offset: None,
            length: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The specification fixes the decomposition and the padding byte values so
    /// that compliant generators emit byte-identical wrappers for one manifest.
    #[test]
    fn padding_uses_the_specified_decomposition() {
        assert_eq!(compute_padding(0).unwrap(), Vec::<u8>::new());
        // b = gap mod 3, a = (gap - 4b) / 3, a bytes of 0x00 then b of 0x10.
        assert_eq!(compute_padding(6).unwrap(), vec![0x00, 0x00]);
        assert_eq!(compute_padding(7).unwrap(), vec![0x00, 0x10]);
        assert_eq!(compute_padding(8).unwrap(), vec![0x10, 0x10]);
        assert_eq!(compute_padding(9).unwrap(), vec![0x00, 0x00, 0x00]);
        // gap = 12 has more than one valid decomposition (four 3-byte selectors
        // or three 4-byte ones); the specified one is four 0x00.
        assert_eq!(compute_padding(12).unwrap(), vec![0x00; 4]);
    }

    #[test]
    fn padding_encodes_to_exactly_the_requested_byte_count() {
        for gap in 6..=200usize {
            let padding = compute_padding(gap).expect("gap >= 6 is representable");
            let encoded: String = padding.iter().map(|&b| byte_to_vs(b)).collect();
            assert_eq!(
                encoded.len(),
                gap,
                "gap {gap} encoded to {} bytes",
                encoded.len()
            );
        }
    }

    #[test]
    fn padded_wrapper_hits_the_deterministic_target_length() {
        for m in [0usize, 1, 17, 200] {
            let manifest = vec![0xABu8; m];
            let target = worst_case_wrapper_byte_length(m);
            let padded =
                encode_wrapper_padded(&manifest, target).expect("target is the worst case");
            assert_eq!(padded.len(), target, "manifest of {m} bytes");
            // Padding is ignored on decode; the manifest round-trips intact.
            let extracted = extract_manifest(&padded).expect("wrapper decodes");
            assert_eq!(extracted.manifest.as_deref(), Some(&manifest[..]));
        }
    }

    #[test]
    fn gaps_that_are_not_representable_are_rejected() {
        // 1, 2 and 5 are not expressible as 3a + 4b. The +6 margin keeps real
        // wrappers clear of them, but the guard must hold.
        for gap in [1usize, 2, 5] {
            assert!(
                compute_padding(gap).is_err(),
                "gap {gap} should be rejected"
            );
        }
    }
}
