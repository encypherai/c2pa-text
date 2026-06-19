"""
Tests for the C2PA manifest validator.
"""

import struct
import unicodedata

from c2pa_text import (
    MAGIC,
    VERSION,
    ValidationCode,
    ValidationResult,
    embed_manifest,
    extract_manifest,
    find_wrapper_info,
    validate_jumbf_structure,
    validate_manifest,
    validate_text,
    validate_wrapper_bytes,
)


class TestValidateManifest:
    """Tests for validate_manifest() - the main validation entry point."""

    def test_empty_manifest_fails(self):
        """Empty bytes should fail validation."""
        result = validate_manifest(b"")
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_minimal_valid_jumbf(self):
        """A minimal valid JUMBF box should pass basic validation."""
        # Minimal JUMBF superbox: size (4) + type (4) = 8 bytes
        # Size of 8 means just the header, no content
        jumbf = struct.pack(">I", 8) + b"jumb"
        result = validate_manifest(jumbf)
        assert result.valid
        assert result.primary_code == ValidationCode.VALID

    def test_invalid_box_type_fails(self):
        """Non-JUMBF box type should fail."""
        # Valid box structure but wrong type
        invalid = struct.pack(">I", 8) + b"xxxx"
        result = validate_manifest(invalid)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_truncated_jumbf_fails(self):
        """JUMBF with declared size larger than actual should fail."""
        # Declare 100 bytes but only provide 8
        truncated = struct.pack(">I", 100) + b"jumb"
        result = validate_manifest(truncated)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_box_size_too_small_fails(self):
        """Box size less than 8 (except 0 and 1) should fail."""
        invalid = struct.pack(">I", 5) + b"jumb"
        result = validate_manifest(invalid)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_extended_size_box(self):
        """Extended size (64-bit) boxes should be handled."""
        # Size = 1 indicates extended size follows
        # Extended size = 24 (16 byte header + 8 bytes content)
        extended = struct.pack(">I", 1) + b"jumb" + struct.pack(">Q", 24) + b"content!"
        result = validate_manifest(extended)
        assert result.valid

    def test_extended_size_truncated_fails(self):
        """Extended size box without enough bytes for 64-bit size should fail."""
        # Size = 1 but only 10 bytes total (need 16 minimum)
        truncated = struct.pack(">I", 1) + b"jumb" + b"xx"
        result = validate_manifest(truncated)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER


class TestValidateJumbfStructure:
    """Tests for validate_jumbf_structure() with strict mode."""

    def test_strict_requires_description_box(self):
        """Strict mode should check for description box."""
        # Just a superbox with no content
        jumbf = struct.pack(">I", 8) + b"jumb"
        result = validate_jumbf_structure(jumbf, strict=True)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_strict_validates_description_box_type(self):
        """Strict mode should validate description box type is 'jumd'."""
        # Superbox with wrong inner box type
        inner = struct.pack(">I", 8) + b"xxxx"
        jumbf = struct.pack(">I", 8 + len(inner)) + b"jumb" + inner
        result = validate_jumbf_structure(jumbf, strict=True)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_strict_with_valid_description_box(self):
        """Strict mode should pass with valid description box."""
        # Description box with C2PA UUID
        c2pa_uuid = bytes.fromhex("6332706100110010800000AA00389B71")
        desc_content = c2pa_uuid + b"\x00" * 8  # UUID + some padding
        desc_box = struct.pack(">I", 8 + len(desc_content)) + b"jumd" + desc_content
        jumbf = struct.pack(">I", 8 + len(desc_box)) + b"jumb" + desc_box
        result = validate_jumbf_structure(jumbf, strict=True)
        assert result.valid


class TestValidateWrapperBytes:
    """Tests for validate_wrapper_bytes() - validates pre-encoded wrappers."""

    def test_valid_wrapper(self):
        """A properly structured wrapper should pass."""
        jumbf = struct.pack(">I", 8) + b"jumb"
        header = struct.pack("!8sBI", MAGIC, VERSION, len(jumbf))
        wrapper = header + jumbf
        result = validate_wrapper_bytes(wrapper)
        assert result.valid
        assert result.version == VERSION
        assert result.declared_length == len(jumbf)

    def test_wrapper_too_short(self):
        """Wrapper shorter than header size should fail."""
        result = validate_wrapper_bytes(b"short")
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_invalid_magic(self):
        """Wrong magic bytes should fail."""
        jumbf = struct.pack(">I", 8) + b"jumb"
        header = struct.pack("!8sBI", b"WRONGMAG", VERSION, len(jumbf))
        wrapper = header + jumbf
        result = validate_wrapper_bytes(wrapper)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_unsupported_version(self):
        """Unsupported version should fail."""
        jumbf = struct.pack(">I", 8) + b"jumb"
        header = struct.pack("!8sBI", MAGIC, 99, len(jumbf))
        wrapper = header + jumbf
        result = validate_wrapper_bytes(wrapper)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_length_mismatch_truncated(self):
        """Declared length larger than actual triggers truncation error."""
        jumbf = struct.pack(">I", 8) + b"jumb"
        # Declare 100 bytes but only provide 8
        header = struct.pack("!8sBI", MAGIC, VERSION, 100)
        wrapper = header + jumbf
        result = validate_wrapper_bytes(wrapper)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

    def test_trailing_padding_accepted(self):
        """Extra bytes after declared manifest length are padding, not an error."""
        jumbf = struct.pack(">I", 8) + b"jumb"
        header = struct.pack("!8sBI", MAGIC, VERSION, len(jumbf))
        # Add 46 bytes of padding after the declared manifest
        padding = b"\x00" * 46
        wrapper = header + jumbf + padding
        result = validate_wrapper_bytes(wrapper)
        assert result.valid, f"Trailing padding should not cause failure: {result}"


class TestValidationResult:
    """Tests for ValidationResult dataclass."""

    def test_str_representation_valid(self):
        """Valid result should have clear string representation."""
        result = ValidationResult(valid=True)
        assert "passed" in str(result).lower()

    def test_str_representation_invalid(self):
        """Invalid result should list issues."""
        result = ValidationResult(valid=True)
        result.add_issue(ValidationCode.CORRUPTED_WRAPPER, "Test message")
        output = str(result)
        assert "failed" in output.lower()
        assert "corruptedWrapper" in output

    def test_add_issue_sets_valid_false(self):
        """Adding an issue should set valid to False."""
        result = ValidationResult(valid=True)
        assert result.valid
        result.add_issue(ValidationCode.CORRUPTED_WRAPPER, "Test")
        assert not result.valid


class TestIntegration:
    """Integration tests combining validation with embed/extract."""

    def test_validate_before_embed(self):
        """Demonstrate validation workflow before embedding."""
        # Create a valid JUMBF-like structure
        jumbf = struct.pack(">I", 8) + b"jumb"

        # Validate first
        result = validate_manifest(jumbf)
        assert result.valid, f"Validation failed: {result}"

        # Then embed
        text = "Hello, World!"
        watermarked = embed_manifest(text, jumbf)

        # Extract and verify
        extracted, clean = extract_manifest(watermarked)
        assert extracted == jumbf
        assert clean == text

    def test_invalid_manifest_caught_before_embed(self):
        """Invalid manifest should be caught by validation."""
        # Truncated/invalid JUMBF
        invalid = struct.pack(">I", 100) + b"jumb"  # Claims 100 bytes, has 8

        result = validate_manifest(invalid)
        assert not result.valid
        assert result.primary_code == ValidationCode.CORRUPTED_WRAPPER

        # Developer would see this and fix before embedding
        assert "truncated" in str(result).lower()


class TestWrapperOffsets:
    def test_wrapper_offsets_are_nfc_utf8_byte_offsets(self):
        jumbf = struct.pack(">I", 8) + b"jumb"

        decomposed = "e\u0301"
        embedded = embed_manifest(decomposed, jumbf)

        info = find_wrapper_info(embedded)
        assert info is not None
        extracted, offset, length = info[0], info[1], info[2]
        assert extracted == jumbf

        normalized_nfc = unicodedata.normalize("NFC", decomposed)
        expected_offset = len(normalized_nfc.encode("utf-8"))
        expected_length = len(embedded.encode("utf-8")) - expected_offset

        assert offset == expected_offset
        assert length == expected_length

        extracted2, clean = extract_manifest(embedded)
        assert extracted2 == jumbf
        assert clean == normalized_nfc


class TestValidateText:
    """Tests for validate_text() - full text asset validation."""

    def _make_signed_text(self, text="Hello, World!"):
        """Helper: create a signed text with a minimal valid JUMBF wrapper."""
        jumbf = struct.pack(">I", 8) + b"jumb"
        return embed_manifest(text, jumbf), jumbf

    def test_plain_text_no_wrapper(self):
        """Plain text with no wrapper is valid (wrapper is optional)."""
        result = validate_text("Just plain text, no C2PA wrapper.")
        assert result.valid
        assert len(result.issues) == 0

    def test_single_valid_wrapper(self):
        """Text with one valid wrapper passes."""
        signed, _ = self._make_signed_text()
        result = validate_text(signed)
        assert result.valid
        assert len(result.issues) == 0

    def test_multiple_wrappers_detected(self):
        """Duplicating the wrapper triggers manifest.text.multipleWrappers."""
        signed, jumbf = self._make_signed_text()
        # Duplicate the wrapper at the end
        from c2pa_text import encode_wrapper

        doubled = signed + encode_wrapper(jumbf)
        result = validate_text(doubled)
        assert not result.valid
        codes = [issue.code for issue in result.issues]
        assert ValidationCode.MULTIPLE_WRAPPERS in codes

    def test_corrupted_wrapper_detected(self):
        """A wrapper with truncated JUMBF triggers a structural failure."""
        # Build a wrapper that declares 100 bytes but has only 8
        truncated_jumbf = struct.pack(">I", 100) + b"jumb"
        header = struct.pack("!8sBI", MAGIC, VERSION, len(truncated_jumbf))
        raw = header + truncated_jumbf
        # Encode as VS
        from c2pa_text import ZWNBSP, _byte_to_vs

        wrapper_str = ZWNBSP + "".join(_byte_to_vs(b) for b in raw)
        text_with_bad_wrapper = "Some text." + wrapper_str
        result = validate_text(text_with_bad_wrapper)
        assert not result.valid
        codes = [issue.code for issue in result.issues]
        assert ValidationCode.CORRUPTED_WRAPPER in codes

    def test_bad_magic_ignored(self):
        """A VS block with wrong magic is not a C2PA wrapper - no error."""
        from c2pa_text import ZWNBSP, _byte_to_vs

        # Build something that looks like a wrapper but has wrong magic
        bad_magic = b"NOTC2PA\0"
        jumbf = struct.pack(">I", 8) + b"jumb"
        header = struct.pack("!8sBI", bad_magic, VERSION, len(jumbf))
        raw = header + jumbf
        wrapper_str = ZWNBSP + "".join(_byte_to_vs(b) for b in raw)
        text = "Some text." + wrapper_str
        result = validate_text(text)
        # Wrong magic means it is not recognized as a C2PA wrapper at all
        assert result.valid

    def test_bad_version_in_wrapper(self):
        """A wrapper with unsupported version maps to corruptedWrapper."""
        from c2pa_text import ZWNBSP, _byte_to_vs

        jumbf = struct.pack(">I", 8) + b"jumb"
        header = struct.pack("!8sBI", MAGIC, 99, len(jumbf))
        raw = header + jumbf
        wrapper_str = ZWNBSP + "".join(_byte_to_vs(b) for b in raw)
        text = "Some text." + wrapper_str
        result = validate_text(text)
        assert not result.valid
        codes = [issue.code for issue in result.issues]
        assert ValidationCode.CORRUPTED_WRAPPER in codes

    def test_length_mismatch_in_wrapper(self):
        """A wrapper declaring more bytes than available maps to corruptedWrapper."""
        from c2pa_text import ZWNBSP, _byte_to_vs

        jumbf = struct.pack(">I", 8) + b"jumb"
        # Declare 50 bytes but only provide 8 (truncated)
        header = struct.pack("!8sBI", MAGIC, VERSION, 50)
        raw = header + jumbf
        wrapper_str = ZWNBSP + "".join(_byte_to_vs(b) for b in raw)
        text = "Some text." + wrapper_str
        result = validate_text(text)
        assert not result.valid
        codes = [issue.code for issue in result.issues]
        assert ValidationCode.CORRUPTED_WRAPPER in codes

    def test_nfc_normalization_applied(self):
        """Validation normalizes to NFC before scanning."""
        decomposed = "e\u0301"  # e + combining acute
        signed, _ = self._make_signed_text(decomposed)
        result = validate_text(signed)
        assert result.valid


class TestRegisteredStatusCodes:
    """Every emitted validation status code must be a registered C2PA code."""

    REGISTERED = {
        "manifest.text.corruptedWrapper",
        "manifest.text.multipleWrappers",
    }

    def _assert_registered(self, result):
        for issue in result.issues:
            assert issue.code.value in self.REGISTERED, f"unregistered status code emitted: {issue.code.value}"

    def test_manifest_and_jumbf_failures_are_registered(self):
        cases = [
            validate_manifest(b""),  # empty
            validate_manifest(struct.pack(">I", 100) + b"jumb"),  # truncated
            validate_manifest(struct.pack(">I", 8) + b"xxxx"),  # bad box type
            validate_manifest(struct.pack(">I", 5) + b"jumb"),  # bad box size
            validate_jumbf_structure(struct.pack(">I", 8) + b"jumb", strict=True),  # missing desc box
        ]
        saw_failure = False
        for result in cases:
            assert not result.valid
            saw_failure = saw_failure or bool(result.issues)
            self._assert_registered(result)
        assert saw_failure

    def test_wrapper_failures_are_registered(self):
        jumbf = struct.pack(">I", 8) + b"jumb"
        cases = [
            validate_wrapper_bytes(b"short"),  # too short
            validate_wrapper_bytes(struct.pack("!8sBI", b"WRONGMAG", VERSION, len(jumbf)) + jumbf),  # bad magic
            validate_wrapper_bytes(struct.pack("!8sBI", MAGIC, 99, len(jumbf)) + jumbf),  # bad version
            validate_wrapper_bytes(struct.pack("!8sBI", MAGIC, VERSION, 100) + jumbf),  # length mismatch
        ]
        for result in cases:
            assert not result.valid
            self._assert_registered(result)

    def test_text_failures_are_registered(self):
        from c2pa_text import ZWNBSP, _byte_to_vs, encode_wrapper

        jumbf = struct.pack(">I", 8) + b"jumb"
        # Multiple wrappers -> manifest.text.multipleWrappers
        signed = embed_manifest("Hello", jumbf)
        doubled = signed + encode_wrapper(jumbf)
        multi = validate_text(doubled)
        assert not multi.valid
        assert ValidationCode.MULTIPLE_WRAPPERS in [i.code for i in multi.issues]
        self._assert_registered(multi)

        # Corrupted wrapper inside text (bad version) -> manifest.text.corruptedWrapper
        header = struct.pack("!8sBI", MAGIC, 99, len(jumbf))
        raw = header + jumbf
        wrapper_str = ZWNBSP + "".join(_byte_to_vs(b) for b in raw)
        bad = validate_text("Some text." + wrapper_str)
        assert not bad.valid
        self._assert_registered(bad)
