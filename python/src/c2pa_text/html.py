"""C2PA HTML embedding (C2PA Technical Specification 2.4, Appendix A.7).

Associates a C2PA Manifest Store with an HTML document using one of the two
methods the spec defines, both keyed on the IANA media type ``application/c2pa``:

* **Inline** -- a ``<script type="application/c2pa">`` element in the ``<head>``
  whose content is the Base64-encoded Manifest Store.
* **Referenced** (preferred) -- a ``<link rel="c2pa-manifest" href="...">``
  element in the ``<head>`` pointing at an external Manifest Store.

A document shall carry at most one association; encountering more than one is the
``manifest.html.multipleManifests`` validation failure (spec A.7.1).

This is a separate pipeline from the unstructured (A.8) and structured (A.9)
methods. Wire-compatible (byte-identical output) with the Rust, TypeScript and
Go ``c2pa-text`` HTML modules for the same inputs.
"""

from __future__ import annotations

import base64
import binascii
from dataclasses import dataclass
from typing import List, Optional

C2PA_MEDIA_TYPE = "application/c2pa"
SCRIPT_OPEN = '<script type="application/c2pa">'
SCRIPT_CLOSE = "</script>"
HEAD_CLOSE = "</head>"

# Validation status code (spec A.7.1).
MULTIPLE_MANIFESTS = "manifest.html.multipleManifests"
# Embed-time error (not a C2PA validation status): the host document has no
# </head> in which to place the manifest element.
NO_HEAD = "html.noHead"


class HtmlError(Exception):
    """HTML embedding/extraction error. ``code`` is the C2PA status code for
    validation failures (e.g. ``manifest.html.multipleManifests``) or an
    embed-time code (``html.noHead``)."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


@dataclass
class HtmlEmbed:
    """Result of embedding an inline manifest into HTML."""

    html: str
    """The document with the ``<script>`` manifest element inserted."""
    exclusion_start: int
    """Byte offset of the ``c2pa.hash.data`` exclusion range (spec A.7.1.3)."""
    exclusion_length: int
    """Byte length of the exclusion range (the entire ``<script>`` element)."""


@dataclass
class HtmlExtraction:
    """Result of extracting a manifest association from HTML."""

    method: str
    """Either ``"inline"`` (script element) or ``"reference"`` (link element)."""
    manifest: Optional[bytes]
    """Decoded Manifest Store bytes, present only for the inline method."""
    reference: Optional[str]
    """External manifest URL, present only for the reference method."""


def build_html_script(manifest_bytes: bytes) -> str:
    """Build a ``<script type="application/c2pa">...</script>`` element whose
    content is the Base64-encoded Manifest Store (spec A.7.1.1)."""
    return SCRIPT_OPEN + base64.b64encode(manifest_bytes).decode("ascii") + SCRIPT_CLOSE


def build_html_link(url: str) -> str:
    """Build a ``<link rel="c2pa-manifest" href="..." type="application/c2pa">``
    element referencing an external Manifest Store (spec A.7.1.2)."""
    return f'<link rel="c2pa-manifest" href="{url}" type="application/c2pa">'


def _insert_before_head_close(html: str, element: str, newline: str) -> int:
    idx = html.find(HEAD_CLOSE)
    if idx == -1:
        raise HtmlError(NO_HEAD, "No </head> found to place the C2PA manifest element")
    return idx


def embed_html_inline(html: str, manifest_bytes: bytes, newline: str = "\n") -> HtmlEmbed:
    """Embed a Manifest Store inline as a ``<script>`` element placed just before
    ``</head>`` and return the document plus the ``c2pa.hash.data`` exclusion
    range covering the element (spec A.7.1.1, A.7.1.3)."""
    element = build_html_script(manifest_bytes)
    idx = _insert_before_head_close(html, element, newline)
    out = html[:idx] + element + newline + html[idx:]
    return HtmlEmbed(
        html=out,
        exclusion_start=len(html[:idx].encode("utf-8")),
        exclusion_length=len(element.encode("utf-8")),
    )


def embed_html_reference(html: str, url: str, newline: str = "\n") -> str:
    """Embed a reference to an external Manifest Store as a ``<link>`` element
    placed just before ``</head>`` (spec A.7.1.2). The hard binding for the
    referenced method has no exclusion range (the hash covers the whole
    document), so no exclusion is returned."""
    element = build_html_link(url)
    idx = _insert_before_head_close(html, element, newline)
    return html[:idx] + element + newline + html[idx:]


def _find_script_contents(html: str) -> List[str]:
    """Return the text content of every ``<script type="application/c2pa">``
    element (form-tolerant: any attribute order, as long as the marker attribute
    is present in the opening tag)."""
    results: List[str] = []
    pos = 0
    while True:
        i = html.find("<script", pos)
        if i == -1:
            break
        gt = html.find(">", i)
        if gt == -1:
            break
        tag = html[i : gt + 1]
        if 'type="application/c2pa"' in tag:
            end = html.find(SCRIPT_CLOSE, gt + 1)
            if end != -1:
                results.append(html[gt + 1 : end])
                pos = end + len(SCRIPT_CLOSE)
                continue
        pos = gt + 1
    return results


def _find_link_tags(html: str) -> List[str]:
    """Return every ``<link ... rel="c2pa-manifest" ...>`` opening tag."""
    results: List[str] = []
    pos = 0
    while True:
        i = html.find("<link", pos)
        if i == -1:
            break
        gt = html.find(">", i)
        if gt == -1:
            break
        tag = html[i : gt + 1]
        if 'rel="c2pa-manifest"' in tag:
            results.append(tag)
        pos = gt + 1
    return results


def _href(tag: str) -> Optional[str]:
    marker = 'href="'
    i = tag.find(marker)
    if i == -1:
        return None
    start = i + len(marker)
    end = tag.find('"', start)
    if end == -1:
        return None
    return tag[start:end]


def extract_html(html: str) -> Optional[HtmlExtraction]:
    """Extract a manifest association from an HTML document (spec A.7.1.4).

    Returns ``None`` if no C2PA association is present. Raises
    :class:`HtmlError` with code ``manifest.html.multipleManifests`` if more than
    one association (script and/or link) is found.
    """
    scripts = _find_script_contents(html)
    links = _find_link_tags(html)
    total = len(scripts) + len(links)
    if total == 0:
        return None
    if total > 1:
        raise HtmlError(MULTIPLE_MANIFESTS, "More than one C2PA manifest association in HTML document")
    if scripts:
        content = scripts[0].strip()
        try:
            manifest: Optional[bytes] = base64.b64decode(content, validate=True)
        except (binascii.Error, ValueError):
            manifest = None
        return HtmlExtraction(method="inline", manifest=manifest, reference=None)
    return HtmlExtraction(method="reference", manifest=None, reference=_href(links[0]))


__all__ = [
    "C2PA_MEDIA_TYPE",
    "MULTIPLE_MANIFESTS",
    "NO_HEAD",
    "HtmlError",
    "HtmlEmbed",
    "HtmlExtraction",
    "build_html_script",
    "build_html_link",
    "embed_html_inline",
    "embed_html_reference",
    "extract_html",
]
