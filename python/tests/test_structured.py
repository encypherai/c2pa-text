"""Tests for the structured-text embedding pipeline (C2PA 2.4 Appendix A.9).

Covers the ASCII Armour block format, single-line and front-matter forms,
data: URI round-trips, hard-binding exclusion ranges, extraction error codes,
and the advisory media-type recommendation.
"""

import pytest

from c2pa_text import (
    BEGIN_DELIMITER,
    END_DELIMITER,
    Method,
    Placement,
    StructuredError,
    build_manifest_block,
    build_manifest_block_multiline,
    decode_data_uri,
    embed_structured,
    encode_data_uri,
    extract_structured,
    recommended_method,
)


class TestDataUri:
    def test_round_trip(self):
        data = bytes([0x01, 0x02, 0x03, 0xFF, 0x00])
        uri = encode_data_uri(data)
        assert uri == "data:application/c2pa;base64,AQID/wA="
        assert decode_data_uri(uri) == data

    def test_non_data_uri_returns_none(self):
        assert decode_data_uri("https://example.com/m.c2pa") is None

    def test_invalid_base64_returns_none(self):
        assert decode_data_uri("data:application/c2pa;base64,!!!!") is None


class TestBlockFormat:
    def test_single_line_no_suffix(self):
        assert build_manifest_block("https://x/m.c2pa", "#") == "# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----"

    def test_single_line_with_suffix(self):
        assert (
            build_manifest_block("https://x/m.c2pa", "<!--", "-->")
            == "<!-- -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST----- -->"
        )

    def test_multiline(self):
        assert build_manifest_block_multiline("https://x/m.c2pa") == "-----BEGIN C2PA MANIFEST-----\nhttps://x/m.c2pa\n-----END C2PA MANIFEST-----"


class TestEmbed:
    def test_start_placement_exclusion_and_round_trip(self):
        block = "# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----"
        r = embed_structured("body line 1\nbody line 2\n", "https://x/m.c2pa", "#", placement=Placement.START)
        assert r.text == f"{block}\nbody line 1\nbody line 2\n"
        assert r.exclusion_start == 0
        assert r.exclusion_length == len((block + "\n").encode("utf-8"))
        excluded = r.text.encode("utf-8")[r.exclusion_start : r.exclusion_start + r.exclusion_length]
        assert excluded == (block + "\n").encode("utf-8")
        x = extract_structured(r.text)
        assert x.reference == "https://x/m.c2pa"
        assert x.manifest is None

    def test_end_placement_exclusion_starts_at_preceding_newline(self):
        text = "#!/usr/bin/env python\nprint('hi')\n"
        block = "# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----"
        r = embed_structured(text, "https://x/m.c2pa", "#", placement=Placement.END)
        assert r.text == f"{text}\n{block}"
        assert r.exclusion_start == len(text.encode("utf-8"))
        assert r.exclusion_length == len(("\n" + block).encode("utf-8"))
        excluded = r.text.encode("utf-8")[r.exclusion_start : r.exclusion_start + r.exclusion_length]
        assert excluded == ("\n" + block).encode("utf-8")

    def test_embed_and_extract_data_uri(self):
        manifest = bytes([0xDE, 0xAD, 0xBE, 0xEF])
        uri = encode_data_uri(manifest)
        r = embed_structured("doc\n", uri, "//")
        x = extract_structured(r.text)
        assert x.reference == uri
        assert x.manifest == manifest

    def test_crlf_newline(self):
        block = "# -----BEGIN C2PA MANIFEST----- u -----END C2PA MANIFEST-----"
        r = embed_structured("a\r\n", "u", "#", newline="\r\n", placement=Placement.START)
        assert r.text == f"{block}\r\na\r\n"
        assert r.exclusion_length == len((block + "\r\n").encode("utf-8"))


class TestExtractErrors:
    def test_no_manifest(self):
        with pytest.raises(StructuredError) as e:
            extract_structured("nothing here")
        assert e.value.code == "manifest.structuredText.noManifest"

    def test_single_delimiter(self):
        with pytest.raises(StructuredError) as e:
            extract_structured(f"# {BEGIN_DELIMITER} https://x")
        assert e.value.code == "manifest.structuredText.noManifest"

    def test_empty_reference(self):
        with pytest.raises(StructuredError) as e:
            extract_structured(f"# {BEGIN_DELIMITER}   {END_DELIMITER}")
        assert e.value.code == "manifest.structuredText.emptyReference"

    def test_multiple_references(self):
        two = f"# {BEGIN_DELIMITER} a {END_DELIMITER}\n# {BEGIN_DELIMITER} b {END_DELIMITER}"
        with pytest.raises(StructuredError) as e:
            extract_structured(two)
        assert e.value.code == "manifest.structuredText.multipleReferences"


class TestFrontMatter:
    def test_front_matter_extracts(self):
        doc = "---\n-----BEGIN C2PA MANIFEST-----\nhttps://x/m.c2pa\n-----END C2PA MANIFEST-----\ntitle: Doc\n---\nbody\n"
        x = extract_structured(doc)
        assert x.reference == "https://x/m.c2pa"


class TestRecommendedMethod:
    @pytest.mark.parametrize(
        "mime,expected",
        [
            ("text/plain", Method.UNSTRUCTURED),
            ("text/markdown", Method.UNSTRUCTURED),
            ("text/csv", Method.UNSTRUCTURED),
            ("application/json", Method.UNSTRUCTURED),
            ("application/xml", Method.STRUCTURED),
            ("text/xml", Method.STRUCTURED),
            ("application/xhtml+xml", Method.STRUCTURED),
            ("text/x-python", Method.STRUCTURED),
            ("text/html", Method.HTML),
            ("image/svg+xml", Method.SVG),
            ("image/jpeg", None),
        ],
    )
    def test_recommendation(self, mime, expected):
        assert recommended_method(mime) == expected
