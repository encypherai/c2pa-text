#!/usr/bin/env python3
"""Generate the cross-language golden fixtures for c2pa-text.

The canonical inputs and expected outputs are computed from the Python
reference implementation (`c2pa_text`), which is verified byte-identical to the
Rust implementation by the parity tests. Every language test suite
(Rust/Python/TypeScript/Go) loads `vectors.json` and asserts byte-for-byte
reproduction, which proves cross-language parity.

Run from the repo root:

    PYTHONPATH=python/src python3 golden/generate.py
"""

import json
import os

from c2pa_text import (
    Placement,
    build_manifest_block,
    build_manifest_block_multiline,
    embed_html_inline,
    embed_html_reference,
    embed_manifest,
    embed_structured,
    encode_data_uri,
)

HERE = os.path.dirname(os.path.abspath(__file__))

# A deterministic 256-byte "Manifest Store" sample used wherever a manifest
# payload is embedded. c2pa-text is manifest-agnostic: it embeds opaque bytes
# (a JUMBF C2PA Manifest Store). See sample.crjson for the human-readable form
# of what such a store represents.
MANIFEST_256 = bytes(range(256))
MANIFEST_SMALL = bytes([0xDE, 0xAD, 0xBE, 0xEF])


def h(s: str) -> str:
    return s.encode("utf-8").hex()


def main() -> None:
    data_uri = []
    for name, payload in [("small", MANIFEST_SMALL), ("all_bytes", MANIFEST_256)]:
        data_uri.append(
            {
                "name": name,
                "manifest_hex": payload.hex(),
                "expected_uri": encode_data_uri(payload),
            }
        )

    structured_block = []
    for name, ref, prefix, suffix in [
        ("python_comment", "https://fabrikam.com/manifests/a1b2c3.c2pa", "#", ""),
        ("js_comment", "https://fabrikam.com/manifests/a1b2c3.c2pa", "//", ""),
        ("sql_comment", "https://fabrikam.com/manifests/a1b2c3.c2pa", "--", ""),
        ("css_block", "https://fabrikam.com/manifests/a1b2c3.c2pa", "/*", "*/"),
        ("xml_comment", "https://fabrikam.com/manifests/a1b2c3.c2pa", "<!--", "-->"),
    ]:
        structured_block.append(
            {
                "name": name,
                "reference": ref,
                "comment_prefix": prefix,
                "comment_suffix": suffix,
                "expected_block": build_manifest_block(ref, prefix, suffix),
            }
        )

    structured_multiline = [
        {
            "name": "front_matter_lf",
            "reference": "https://fabrikam.com/manifests/a1b2c3.c2pa",
            "newline": "\n",
            "expected_block": build_manifest_block_multiline(
                "https://fabrikam.com/manifests/a1b2c3.c2pa", "\n"
            ),
        }
    ]

    embed_cases = [
        # (name, text, reference, prefix, suffix, placement, newline)
        (
            "url_start_lf",
            "Hello, World!\n",
            "https://fabrikam.com/manifests/a1b2c3.c2pa",
            "#",
            "",
            Placement.START,
            "\n",
        ),
        (
            "xml_end_lf",
            '<?xml version="1.0"?>\n<root/>\n',
            "https://fabrikam.com/manifests/a1b2c3.c2pa",
            "<!--",
            "-->",
            Placement.END,
            "\n",
        ),
        (
            "datauri_start_crlf_unicode",
            "café ☕\r\nsecond line\r\n",
            encode_data_uri(MANIFEST_256),
            "//",
            "",
            Placement.START,
            "\r\n",
        ),
    ]
    structured_embed = []
    for name, text, ref, prefix, suffix, placement, newline in embed_cases:
        r = embed_structured(text, ref, prefix, suffix, placement, newline)
        structured_embed.append(
            {
                "name": name,
                "text": text,
                "reference": ref,
                "comment_prefix": prefix,
                "comment_suffix": suffix,
                "placement": placement.value,
                "newline": newline,
                "expected_text_hex": h(r.text),
                "exclusion_start": r.exclusion_start,
                "exclusion_length": r.exclusion_length,
            }
        )

    unstructured_embed = []
    for name, text, payload in [
        ("ascii_small", "hello world", MANIFEST_SMALL),
        ("unicode_all_bytes", "café ☕ provenance", MANIFEST_256),
    ]:
        unstructured_embed.append(
            {
                "name": name,
                "text": text,
                "manifest_hex": payload.hex(),
                "expected_embed_hex": h(embed_manifest(text, payload)),
            }
        )

    html_doc = (
        '<!DOCTYPE html>\n<html lang="en">\n<head>\n'
        '<meta charset="utf-8">\n<title>Example</title>\n</head>\n'
        "<body>\n<p>Content here.</p>\n</body>\n</html>\n"
    )
    he = embed_html_inline(html_doc, MANIFEST_SMALL)
    html_inline = [
        {
            "name": "inline_small",
            "html": html_doc,
            "manifest_hex": MANIFEST_SMALL.hex(),
            "newline": "\n",
            "expected_html_hex": h(he.html),
            "exclusion_start": he.exclusion_start,
            "exclusion_length": he.exclusion_length,
        }
    ]
    html_reference = [
        {
            "name": "reference_url",
            "html": html_doc,
            "url": "https://fabrikam.com/manifest.c2pa",
            "newline": "\n",
            "expected_html_hex": h(
                embed_html_reference(html_doc, "https://fabrikam.com/manifest.c2pa")
            ),
        }
    ]
    vectors = {
        "_comment": (
            "Cross-language golden vectors for c2pa-text. Generated by "
            "golden/generate.py from the Python reference implementation. All "
            "language suites (Rust/Python/TypeScript/Go) assert byte-for-byte "
            "reproduction of these values."
        ),
        "spec": "C2PA Technical Specification 2.4, Appendix A.7 (HTML), A.8 (unstructured), A.9 (structured)",
        "data_uri": data_uri,
        "structured_block": structured_block,
        "structured_multiline": structured_multiline,
        "structured_embed": structured_embed,
        "unstructured_embed": unstructured_embed,
        "html_inline": html_inline,
        "html_reference": html_reference,
    }

    with open(os.path.join(HERE, "vectors.json"), "w", encoding="utf-8") as f:
        json.dump(vectors, f, indent=2, ensure_ascii=False)
        f.write("\n")

    # Binary sample manifest store (opaque bytes used by the vectors).
    with open(os.path.join(HERE, "manifest_store.bin"), "wb") as f:
        f.write(MANIFEST_256)

    # Human-readable golden sample files (real embedded outputs).
    samples = os.path.join(HERE, "samples")
    os.makedirs(samples, exist_ok=True)

    # Unstructured (A.8): variation-selector wrapper appended to plain text.
    with open(os.path.join(samples, "note.txt"), "w", encoding="utf-8") as f:
        f.write(embed_manifest("This note carries Content Credentials.\n", MANIFEST_SMALL))

    # Structured (A.9): single-line comment, external URL reference, at start.
    py = embed_structured(
        "import sys\n\nprint('hello')\n",
        "https://fabrikam.com/manifests/a1b2c3.c2pa",
        "#",
        "",
        Placement.START,
        "\n",
    )
    with open(os.path.join(samples, "script.py"), "w", encoding="utf-8") as f:
        f.write(py.text)

    # Structured (A.9): Markdown comment with inline data: URI, at start.
    md = embed_structured(
        "# Title\n\nBody text.\n",
        encode_data_uri(MANIFEST_SMALL),
        "<!--",
        "-->",
        Placement.START,
        "\n",
    )
    with open(os.path.join(samples, "document.md"), "w", encoding="utf-8") as f:
        f.write(md.text)

    # HTML (A.7): inline <script type="application/c2pa"> in the head.
    with open(os.path.join(samples, "page.html"), "w", encoding="utf-8") as f:
        f.write(he.html)
    print("Wrote vectors.json, manifest_store.bin, and samples/.")


if __name__ == "__main__":
    main()
