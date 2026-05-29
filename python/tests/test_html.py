"""Tests for the HTML embedding pipeline (C2PA 2.4 Appendix A.7)."""

import json
from pathlib import Path

import pytest

from c2pa_text import (
    HtmlError,
    build_html_link,
    build_html_script,
    embed_html_inline,
    embed_html_reference,
    extract_html,
)

HTML = (
    '<!DOCTYPE html>\n<html lang="en">\n<head>\n'
    '<meta charset="utf-8">\n<title>Example</title>\n</head>\n'
    "<body>\n<p>Content here.</p>\n</body>\n</html>\n"
)

GOLDEN = json.loads((Path(__file__).resolve().parents[2] / "golden" / "vectors.json").read_text(encoding="utf-8"))


def _hex(s: str) -> str:
    return s.encode("utf-8").hex()


class TestBuilders:
    def test_script(self):
        assert build_html_script(bytes([0xDE, 0xAD, 0xBE, 0xEF])) == '<script type="application/c2pa">3q2+7w==</script>'

    def test_link(self):
        assert build_html_link("https://x/m.c2pa") == '<link rel="c2pa-manifest" href="https://x/m.c2pa" type="application/c2pa">'


class TestInline:
    def test_exclusion_and_round_trip(self):
        manifest = bytes([0xDE, 0xAD, 0xBE, 0xEF])
        r = embed_html_inline(HTML, manifest)
        element = '<script type="application/c2pa">3q2+7w==</script>'
        # Exclusion covers exactly the inserted <script> element.
        excluded = r.html.encode("utf-8")[r.exclusion_start : r.exclusion_start + r.exclusion_length]
        assert excluded == element.encode("utf-8")
        # Element sits inside the head, before </head>.
        assert element + "\n</head>" in r.html
        x = extract_html(r.html)
        assert x is not None
        assert x.method == "inline"
        assert x.manifest == manifest

    def test_no_head_raises(self):
        with pytest.raises(HtmlError) as e:
            embed_html_inline("<p>no head here</p>", b"\x00")
        assert e.value.code == "html.noHead"


class TestReference:
    def test_round_trip(self):
        url = "https://fabrikam.com/manifest.c2pa"
        html = embed_html_reference(HTML, url)
        x = extract_html(html)
        assert x is not None
        assert x.method == "reference"
        assert x.reference == url
        assert x.manifest is None


class TestExtract:
    def test_none(self):
        assert extract_html(HTML) is None

    def test_multiple_manifests(self):
        r = embed_html_inline(HTML, b"\x00")
        # Add a second association (a link) -> more than one.
        doubled = embed_html_reference(r.html, "https://x/m.c2pa")
        with pytest.raises(HtmlError) as e:
            extract_html(doubled)
        assert e.value.code == "manifest.html.multipleManifests"


class TestGolden:
    @pytest.mark.parametrize("v", GOLDEN["html_inline"], ids=lambda v: v["name"])
    def test_inline(self, v):
        manifest = bytes.fromhex(v["manifest_hex"])
        r = embed_html_inline(v["html"], manifest, v["newline"])
        assert _hex(r.html) == v["expected_html_hex"]
        assert r.exclusion_start == v["exclusion_start"]
        assert r.exclusion_length == v["exclusion_length"]
        assert extract_html(r.html).manifest == manifest

    @pytest.mark.parametrize("v", GOLDEN["html_reference"], ids=lambda v: v["name"])
    def test_reference(self, v):
        html = embed_html_reference(v["html"], v["url"], v["newline"])
        assert _hex(html) == v["expected_html_hex"]
        assert extract_html(html).reference == v["url"]
