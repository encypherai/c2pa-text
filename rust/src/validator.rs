//! C2PA Manifest Structural Validator.
//!
//! Provides validation utilities to help developers ensure their C2PA manifests
//! are structurally compliant before embedding them into text.

use std::fmt;

/// JUMBF Constants (ISO/IEC 19566-5)
const JUMBF_SUPERBOX_TYPE: &[u8; 4] = b"jumb";
const JUMBF_DESC_TYPE: &[u8; 4] = b"jumd";
const C2PA_MANIFEST_STORE_UUID: [u8; 16] = [
    0x63, 0x32, 0x70, 0x61, 0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

/// C2PA-compliant validation status codes for text manifests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationCode {
    /// Manifest is valid
    Valid,
    /// Wrapper-level failures (from C2PA Text spec)
    CorruptedWrapper,
    MultipleWrappers,
    /// Extended validation codes
    InvalidMagic,
    UnsupportedVersion,
    LengthMismatch,
    EmptyManifest,
    /// JUMBF-level failures
    InvalidJumbfHeader,
    InvalidJumbfBoxSize,
    MissingDescriptionBox,
    InvalidC2paUuid,
    TruncatedJumbf,
}

impl ValidationCode {
    /// Returns the C2PA-compliant status code string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidationCode::Valid => "valid",
            ValidationCode::CorruptedWrapper => "manifest.text.corruptedWrapper",
            ValidationCode::MultipleWrappers => "manifest.text.multipleWrappers",
            ValidationCode::InvalidMagic => "manifest.text.invalidMagic",
            ValidationCode::UnsupportedVersion => "manifest.text.unsupportedVersion",
            ValidationCode::LengthMismatch => "manifest.text.lengthMismatch",
            ValidationCode::EmptyManifest => "manifest.text.emptyManifest",
            ValidationCode::InvalidJumbfHeader => "manifest.jumbf.invalidHeader",
            ValidationCode::InvalidJumbfBoxSize => "manifest.jumbf.invalidBoxSize",
            ValidationCode::MissingDescriptionBox => "manifest.jumbf.missingDescriptionBox",
            ValidationCode::InvalidC2paUuid => "manifest.jumbf.invalidC2paUuid",
            ValidationCode::TruncatedJumbf => "manifest.jumbf.truncated",
        }
    }
}

impl fmt::Display for ValidationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single validation issue with location and details.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub code: ValidationCode,
    pub message: String,
    pub offset: Option<usize>,
    pub context: Option<String>,
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

/// Result of manifest validation with detailed diagnostics.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
    pub manifest_bytes: Option<Vec<u8>>,
    pub jumbf_bytes: Option<Vec<u8>>,
    pub version: Option<u8>,
    pub declared_length: Option<u32>,
    pub actual_length: Option<usize>,
}

impl ValidationResult {
    /// Create a new valid result.
    pub fn new() -> Self {
        Self {
            valid: true,
            issues: Vec::new(),
            manifest_bytes: None,
            jumbf_bytes: None,
            version: None,
            declared_length: None,
            actual_length: None,
        }
    }

    /// Add a validation issue.
    pub fn add_issue(
        &mut self,
        code: ValidationCode,
        message: impl Into<String>,
        offset: Option<usize>,
        context: Option<String>,
    ) {
        self.issues.push(ValidationIssue {
            code,
            message: message.into(),
            offset,
            context,
        });
        self.valid = false;
    }

    /// Returns the most severe validation code.
    pub fn primary_code(&self) -> ValidationCode {
        self.issues
            .first()
            .map(|i| i.code.clone())
            .unwrap_or(ValidationCode::Valid)
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.valid {
            write!(f, "Validation passed: manifest is structurally compliant")
        } else {
            writeln!(f, "Validation failed:")?;
            for issue in &self.issues {
                writeln!(f, "  - {}", issue)?;
            }
            Ok(())
        }
    }
}

/// Validate basic JUMBF box structure.
pub fn validate_jumbf_structure(jumbf_bytes: &[u8], strict: bool) -> ValidationResult {
    let mut result = ValidationResult::new();
    result.jumbf_bytes = Some(jumbf_bytes.to_vec());

    if jumbf_bytes.is_empty() {
        result.add_issue(
            ValidationCode::EmptyManifest,
            "JUMBF content is empty",
            Some(0),
            None,
        );
        return result;
    }

    // Minimum JUMBF box: 8 bytes header (size + type)
    if jumbf_bytes.len() < 8 {
        result.add_issue(
            ValidationCode::InvalidJumbfHeader,
            format!(
                "JUMBF too short for box header: {} bytes, minimum 8",
                jumbf_bytes.len()
            ),
            Some(0),
            None,
        );
        return result;
    }

    // Parse first box header
    let box_size = u32::from_be_bytes([
        jumbf_bytes[0],
        jumbf_bytes[1],
        jumbf_bytes[2],
        jumbf_bytes[3],
    ]);
    let box_type = &jumbf_bytes[4..8];

    // Validate box size
    let (effective_size, header_size) = if box_size == 0 {
        // Size 0 means "extends to end of file"
        (jumbf_bytes.len(), 8)
    } else if box_size == 1 {
        // Extended size (64-bit)
        if jumbf_bytes.len() < 16 {
            result.add_issue(
                ValidationCode::TruncatedJumbf,
                "Extended box size declared but not enough bytes for 64-bit size field",
                Some(0),
                None,
            );
            return result;
        }
        let extended_size = u64::from_be_bytes([
            jumbf_bytes[8],
            jumbf_bytes[9],
            jumbf_bytes[10],
            jumbf_bytes[11],
            jumbf_bytes[12],
            jumbf_bytes[13],
            jumbf_bytes[14],
            jumbf_bytes[15],
        ]) as usize;
        (extended_size, 16)
    } else if box_size < 8 {
        result.add_issue(
            ValidationCode::InvalidJumbfBoxSize,
            format!("Invalid box size: {} (minimum is 8)", box_size),
            Some(0),
            None,
        );
        return result;
    } else {
        (box_size as usize, 8)
    };

    // Check if we have enough bytes
    if jumbf_bytes.len() < effective_size {
        result.add_issue(
            ValidationCode::TruncatedJumbf,
            format!(
                "JUMBF truncated: declared size {}, actual {}",
                effective_size,
                jumbf_bytes.len()
            ),
            Some(0),
            None,
        );
        return result;
    }

    // Check for JUMBF superbox type
    if box_type != JUMBF_SUPERBOX_TYPE {
        result.add_issue(
            ValidationCode::InvalidJumbfHeader,
            format!(
                "Expected JUMBF superbox type 'jumb', got '{}'",
                String::from_utf8_lossy(box_type)
            ),
            Some(4),
            Some(format!("box_type={:02x?}", box_type)),
        );
        return result;
    }

    if strict {
        // Check for description box (jumd)
        if jumbf_bytes.len() < header_size + 8 {
            result.add_issue(
                ValidationCode::MissingDescriptionBox,
                "JUMBF superbox too short to contain description box",
                Some(header_size),
                None,
            );
            return result;
        }

        let desc_type = &jumbf_bytes[header_size + 4..header_size + 8];
        if desc_type != JUMBF_DESC_TYPE {
            result.add_issue(
                ValidationCode::MissingDescriptionBox,
                format!(
                    "Expected description box 'jumd', got '{}'",
                    String::from_utf8_lossy(desc_type)
                ),
                Some(header_size + 4),
                None,
            );
            return result;
        }

        // Check for C2PA UUID
        let uuid_offset = header_size + 8;
        if jumbf_bytes.len() >= uuid_offset + 16 {
            let found_uuid = &jumbf_bytes[uuid_offset..uuid_offset + 16];
            if found_uuid != C2PA_MANIFEST_STORE_UUID {
                result.add_issue(
                    ValidationCode::InvalidC2paUuid,
                    "Invalid C2PA manifest store UUID",
                    Some(uuid_offset),
                    Some(format!(
                        "expected={:02x?}, found={:02x?}",
                        C2PA_MANIFEST_STORE_UUID, found_uuid
                    )),
                );
            }
        }
    }

    result
}

/// Validate a C2PA manifest before embedding.
///
/// This is the main validation entry point.
pub fn validate_manifest(
    manifest_bytes: &[u8],
    validate_jumbf: bool,
    strict: bool,
) -> ValidationResult {
    let mut result = ValidationResult::new();
    result.manifest_bytes = Some(manifest_bytes.to_vec());

    if manifest_bytes.is_empty() {
        result.add_issue(
            ValidationCode::EmptyManifest,
            "Manifest bytes are empty",
            None,
            None,
        );
        return result;
    }

    result.actual_length = Some(manifest_bytes.len());

    if validate_jumbf {
        let jumbf_result = validate_jumbf_structure(manifest_bytes, strict);
        if !jumbf_result.valid {
            result.issues.extend(jumbf_result.issues);
            result.valid = false;
        }
    }

    result
}

/// Validate a pre-encoded C2PATextManifestWrapper.
pub fn validate_wrapper_bytes(wrapper_bytes: &[u8]) -> ValidationResult {
    use crate::{HEADER_SIZE, MAGIC, VERSION};

    let mut result = ValidationResult::new();

    if wrapper_bytes.len() < HEADER_SIZE {
        result.add_issue(
            ValidationCode::CorruptedWrapper,
            format!(
                "Wrapper too short: {} bytes, minimum {}",
                wrapper_bytes.len(),
                HEADER_SIZE
            ),
            Some(0),
            None,
        );
        return result;
    }

    // Check magic
    if &wrapper_bytes[0..8] != MAGIC {
        result.add_issue(
            ValidationCode::InvalidMagic,
            format!(
                "Invalid magic: expected 'C2PATXT\\0', got {:?}",
                &wrapper_bytes[0..8]
            ),
            Some(0),
            None,
        );
        return result;
    }

    // Check version
    let version = wrapper_bytes[8];
    result.version = Some(version);
    if version != VERSION {
        result.add_issue(
            ValidationCode::UnsupportedVersion,
            format!("Unsupported version: {}, expected {}", version, VERSION),
            Some(8),
            None,
        );
        return result;
    }

    // Check length
    let declared_length = u32::from_be_bytes([
        wrapper_bytes[9],
        wrapper_bytes[10],
        wrapper_bytes[11],
        wrapper_bytes[12],
    ]);
    result.declared_length = Some(declared_length);

    let actual_jumbf_length = wrapper_bytes.len() - HEADER_SIZE;
    result.actual_length = Some(actual_jumbf_length);

    // Actual bytes after header must be >= declared. Trailing bytes beyond
    // manifestLength are padding (spec says decoders use manifestLength to
    // extract the manifest and ignore trailing padding).
    if (declared_length as usize) > actual_jumbf_length {
        result.add_issue(
            ValidationCode::LengthMismatch,
            format!(
                "Length mismatch: declares {} bytes, only {} available (truncated)",
                declared_length, actual_jumbf_length
            ),
            Some(9),
            None,
        );
        return result;
    }

    // Extract the declared manifest bytes (ignore trailing padding)
    let jumbf_bytes = &wrapper_bytes[HEADER_SIZE..HEADER_SIZE + declared_length as usize];
    result.jumbf_bytes = Some(jumbf_bytes.to_vec());
    result.manifest_bytes = Some(jumbf_bytes.to_vec());

    let jumbf_result = validate_jumbf_structure(jumbf_bytes, false);
    if !jumbf_result.valid {
        result.issues.extend(jumbf_result.issues);
        result.valid = false;
    }

    result
}

/// Validate a text asset for C2PA text wrapper compliance.
///
/// Scans the full text for C2PA wrappers and reports structural issues:
/// - Multiple wrappers (spec requires zero or one)
/// - Corrupted, truncated, or malformed wrappers
/// - Invalid magic bytes or unsupported version
/// - JUMBF structural issues in the embedded manifest
///
/// Returns a [`ValidationResult`] with all issues found across all wrappers.
pub fn validate_text(text: &str) -> ValidationResult {
    use crate::{vs_to_byte, HEADER_SIZE, MAGIC, ZWNBSP};
    use unicode_normalization::UnicodeNormalization;

    let mut result = ValidationResult::new();
    let normalized: String = text.nfc().collect();

    // Scan for potential wrappers: ZWNBSP followed by variation selectors.
    let chars: Vec<(usize, char)> = normalized.char_indices().collect();
    let mut valid_wrappers: Vec<(usize, Vec<u8>)> = Vec::new(); // (char_index, raw_bytes)

    let mut i = 0;
    while i < chars.len() {
        let (idx, c) = chars[i];
        if c == ZWNBSP {
            // Decode the VS sequence following the ZWNBSP.
            let mut raw = Vec::new();
            let mut j = i + 1;
            while j < chars.len() {
                if let Some(b) = vs_to_byte(chars[j].1) {
                    raw.push(b);
                    j += 1;
                } else {
                    break;
                }
            }

            // Check for valid C2PA header.
            if raw.len() >= HEADER_SIZE && &raw[0..8] == MAGIC {
                valid_wrappers.push((idx, raw));
            }

            i = j;
            continue;
        }
        i += 1;
    }

    if valid_wrappers.is_empty() {
        // No wrapper found is valid (wrapper is optional per spec).
        return result;
    }

    if valid_wrappers.len() > 1 {
        // Compute byte offset of the second wrapper for diagnostics.
        let second_start = valid_wrappers[1].0;
        let byte_offset = normalized[..second_start].len();
        result.add_issue(
            ValidationCode::MultipleWrappers,
            format!(
                "Found {} valid C2PA text wrappers (spec requires at most one)",
                valid_wrappers.len()
            ),
            Some(byte_offset),
            None,
        );
    }

    // Validate each wrapper structurally.
    for (_char_idx, raw) in &valid_wrappers {
        let wrapper_result = validate_wrapper_bytes(raw);
        if !wrapper_result.valid {
            for issue in wrapper_result.issues {
                result.issues.push(issue);
            }
            result.valid = false;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_manifest_fails() {
        let result = validate_manifest(&[], true, false);
        assert!(!result.valid);
        assert_eq!(result.primary_code(), ValidationCode::EmptyManifest);
    }

    #[test]
    fn test_minimal_valid_jumbf() {
        // Minimal JUMBF superbox: size (4) + type (4) = 8 bytes
        let mut jumbf = vec![0, 0, 0, 8]; // size = 8
        jumbf.extend_from_slice(b"jumb");
        let result = validate_manifest(&jumbf, true, false);
        assert!(result.valid);
    }

    #[test]
    fn test_invalid_box_type_fails() {
        let mut invalid = vec![0, 0, 0, 8];
        invalid.extend_from_slice(b"xxxx");
        let result = validate_manifest(&invalid, true, false);
        assert!(!result.valid);
        assert_eq!(result.primary_code(), ValidationCode::InvalidJumbfHeader);
    }

    #[test]
    fn test_truncated_jumbf_fails() {
        let mut truncated = vec![0, 0, 0, 100]; // claims 100 bytes
        truncated.extend_from_slice(b"jumb");
        let result = validate_manifest(&truncated, true, false);
        assert!(!result.valid);
        assert_eq!(result.primary_code(), ValidationCode::TruncatedJumbf);
    }

    // ---------- validate_text tests ----------

    #[test]
    fn test_validate_text_plain_no_wrapper() {
        let result = validate_text("Just plain text, no C2PA wrapper.");
        assert!(result.valid);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_validate_text_single_valid_wrapper() {
        let jumbf = vec![0u8, 0, 0, 8, b'j', b'u', b'm', b'b'];
        let signed = crate::embed_manifest("Hello, World!", &jumbf);
        let result = validate_text(&signed);
        assert!(
            result.valid,
            "Single valid wrapper should pass: {:?}",
            result
        );
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_validate_text_multiple_wrappers() {
        let jumbf = vec![0u8, 0, 0, 8, b'j', b'u', b'm', b'b'];
        let signed = crate::embed_manifest("Hello!", &jumbf);
        let doubled = format!("{}{}", signed, crate::encode_wrapper(&jumbf));
        let result = validate_text(&doubled);
        assert!(!result.valid);
        let codes: Vec<_> = result.issues.iter().map(|i| &i.code).collect();
        assert!(
            codes.contains(&&ValidationCode::MultipleWrappers),
            "Expected MultipleWrappers, got {:?}",
            codes
        );
    }

    #[test]
    fn test_validate_text_bad_version() {
        use crate::{byte_to_vs, MAGIC, ZWNBSP};
        let jumbf = vec![0u8, 0, 0, 8, b'j', b'u', b'm', b'b'];
        let mut raw = Vec::new();
        raw.extend_from_slice(MAGIC);
        raw.push(99); // bad version
        let len_bytes = (jumbf.len() as u32).to_be_bytes();
        raw.extend_from_slice(&len_bytes);
        raw.extend_from_slice(&jumbf);
        let mut wrapper = String::new();
        wrapper.push(ZWNBSP);
        for &b in &raw {
            wrapper.push(byte_to_vs(b));
        }
        let text = format!("Some text.{}", wrapper);
        let result = validate_text(&text);
        assert!(!result.valid);
        let codes: Vec<_> = result.issues.iter().map(|i| &i.code).collect();
        assert!(codes.contains(&&ValidationCode::UnsupportedVersion));
    }

    #[test]
    fn test_validate_text_length_mismatch() {
        use crate::{byte_to_vs, MAGIC, VERSION, ZWNBSP};
        let jumbf = vec![0u8, 0, 0, 8, b'j', b'u', b'm', b'b'];
        let mut raw = Vec::new();
        raw.extend_from_slice(MAGIC);
        raw.push(VERSION);
        // Declare 50 bytes but only provide 8 (truncated).
        let len_bytes = 50u32.to_be_bytes();
        raw.extend_from_slice(&len_bytes);
        raw.extend_from_slice(&jumbf);
        let mut wrapper = String::new();
        wrapper.push(ZWNBSP);
        for &b in &raw {
            wrapper.push(byte_to_vs(b));
        }
        let text = format!("Some text.{}", wrapper);
        let result = validate_text(&text);
        assert!(!result.valid);
        let codes: Vec<_> = result.issues.iter().map(|i| &i.code).collect();
        assert!(codes.contains(&&ValidationCode::LengthMismatch));
    }

    #[test]
    fn test_validate_text_nfc_normalization() {
        let jumbf = vec![0u8, 0, 0, 8, b'j', b'u', b'm', b'b'];
        let decomposed = "e\u{0301}"; // e + combining acute
        let signed = crate::embed_manifest(decomposed, &jumbf);
        let result = validate_text(&signed);
        assert!(result.valid);
    }

    #[test]
    fn test_validate_text_bad_magic_ignored() {
        use crate::{byte_to_vs, VERSION, ZWNBSP};
        let bad_magic = b"NOTC2PA\0";
        let jumbf = vec![0u8, 0, 0, 8, b'j', b'u', b'm', b'b'];
        let mut raw = Vec::new();
        raw.extend_from_slice(bad_magic);
        raw.push(VERSION);
        let len_bytes = (jumbf.len() as u32).to_be_bytes();
        raw.extend_from_slice(&len_bytes);
        raw.extend_from_slice(&jumbf);
        let mut wrapper = String::new();
        wrapper.push(ZWNBSP);
        for &b in &raw {
            wrapper.push(byte_to_vs(b));
        }
        let text = format!("Some text.{}", wrapper);
        let result = validate_text(&text);
        // Wrong magic means not recognized as a C2PA wrapper at all.
        assert!(result.valid);
    }
}
