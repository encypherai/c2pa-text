"""C2PA Structured Text embedding (C2PA Technical Specification 2.4, Appendix A.9).

Associates a C2PA Manifest Store with a *structured* text asset -- source code,
configuration files (YAML, TOML, INI), markup (Markdown, AsciiDoc, LaTeX), XML,
and similar formats that support a comment or front-matter convention -- using
an ASCII Armour-style block (modelled on OpenPGP ASCII Armor, RFC 4880 sec 6.2)
delimited by::

    -----BEGIN C2PA MANIFEST----- <manifest-reference> -----END C2PA MANIFEST-----

The ``<manifest-reference>`` is either:

* a URL to an external C2PA Manifest Store (preferred), or
* a ``data:application/c2pa;base64,...`` URI embedding the store inline.

This is a separate pipeline from the unstructured (Unicode Variation Selector)
method in :mod:`c2pa_text` (Appendix A.8). Neither pipeline is restricted to a
fixed set of media types: the implementer chooses which method to use for a
given asset. :func:`recommended_method` offers an advisory mapping for the media
types named in the spec, but it is informative only.

Note: ``text/html`` (Appendix A.7) and ``image/svg+xml`` (Appendix A.3.3) have
their own dedicated embedding methods and are out of scope for this pipeline.

This module is wire-compatible (byte-identical output) with the Rust
``c2pa_text::structured`` module for the same inputs.
"""

from __future__ import annotations

import base64
import binascii
from dataclasses import dataclass
from enum import Enum

# Fixed ASCII Armour-style delimiters (spec A.9.3).
BEGIN_DELIMITER = "-----BEGIN C2PA MANIFEST-----"
END_DELIMITER = "-----END C2PA MANIFEST-----"
# Prefix of a data: URI carrying a Base64-encoded C2PA Manifest Store.
DATA_URI_PREFIX = "data:application/c2pa;base64,"


class Placement(str, Enum):
    """Where to place the manifest block relative to the host text (spec A.9.3.1)."""

    START = "start"
    """Beginning of the file (preferred by the spec)."""
    END = "end"
    """End of the file -- used when the first line is reserved by the host
    format (e.g. a ``#!`` shebang or an ``<?xml ?>`` declaration), so the
    ``-----END C2PA MANIFEST-----`` delimiter appears on the last line."""


class Method(str, Enum):
    """Advisory recommendation of which embedding method best fits a media type."""

    UNSTRUCTURED = "unstructured"
    """Unicode Variation Selector wrapper (Appendix A.8, :mod:`c2pa_text`)."""
    STRUCTURED = "structured"
    """ASCII Armour comment/front-matter block (Appendix A.9, this module)."""
    HTML = "html"
    """HTML script/link method (Appendix A.7) -- not implemented by this package."""
    SVG = "svg"
    """SVG metadata method (Appendix A.3.3) -- not implemented by this package."""


@dataclass
class StructuredEmbed:
    """Result of embedding a structured-text manifest block."""

    text: str
    """The host text with the manifest block inserted."""
    exclusion_start: int
    """Byte offset of the ``c2pa.hash.data`` exclusion range (spec A.9.4)."""
    exclusion_length: int
    """Byte length of the ``c2pa.hash.data`` exclusion range (spec A.9.4)."""


@dataclass
class StructuredExtraction:
    """Result of extracting a structured-text manifest block."""

    reference: str
    """The manifest reference between the delimiters (URL or ``data:`` URI), trimmed."""
    manifest: bytes | None
    """Decoded Manifest Store bytes -- present only when ``reference`` is a
    ``data:application/c2pa;base64,...`` URI with a valid Base64 payload."""


class StructuredError(Exception):
    """Raised on structured-text extraction failure.

    The ``code`` attribute is the normative C2PA validation status code
    (spec A.9.5), e.g. ``manifest.structuredText.noManifest``.
    """

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def encode_data_uri(manifest_bytes: bytes) -> str:
    """Build a ``data:application/c2pa;base64,...`` URI for a Manifest Store
    (spec A.9.3.1), using standard Base64 (RFC 4648 sec 4, padded, no line breaks).
    """
    return DATA_URI_PREFIX + base64.b64encode(manifest_bytes).decode("ascii")


def decode_data_uri(reference: str) -> bytes | None:
    """Decode a ``data:application/c2pa;base64,...`` reference into Manifest Store
    bytes. Returns ``None`` if ``reference`` is not such a ``data:`` URI or the
    Base64 payload is invalid.
    """
    if not reference.startswith(DATA_URI_PREFIX):
        return None
    payload = reference[len(DATA_URI_PREFIX) :].strip()
    try:
        return base64.b64decode(payload, validate=True)
    except (binascii.Error, ValueError):
        return None


def build_manifest_block(reference: str, comment_prefix: str, comment_suffix: str = "") -> str:
    """Build a single-line manifest block (spec A.9.3.1)::

        <comment_prefix> -----BEGIN C2PA MANIFEST----- <reference> -----END C2PA MANIFEST----- <comment_suffix>

    ``comment_suffix`` is appended (space-separated) only when non-empty, for
    block-comment formats such as CSS (``/* */``) or XML/Markdown (``<!-- -->``).
    """
    block = f"{comment_prefix} {BEGIN_DELIMITER} {reference} {END_DELIMITER}"
    if comment_suffix:
        block += f" {comment_suffix}"
    return block


def build_manifest_block_multiline(reference: str, newline: str = "\n") -> str:
    """Build a multi-line manifest block for placement inside host front matter
    (spec A.9.3.2)::

        -----BEGIN C2PA MANIFEST-----
        <reference>
        -----END C2PA MANIFEST-----

    The host front-matter fences (e.g. ``---`` for YAML) are part of the host
    format, not the C2PA block, and must be supplied by the caller.
    """
    return f"{BEGIN_DELIMITER}{newline}{reference}{newline}{END_DELIMITER}"


def embed_structured(
    text: str,
    reference: str,
    comment_prefix: str,
    comment_suffix: str = "",
    placement: Placement = Placement.START,
    newline: str = "\n",
) -> StructuredEmbed:
    """Embed a manifest block into structured text using the single-line comment
    form (spec A.9.3.1) and return the resulting text together with the
    ``c2pa.hash.data`` exclusion range to bind it (spec A.9.4).

    ``newline`` is the host file's line terminator -- ``"\\n"`` (LF) or
    ``"\\r\\n"`` (CRLF); bare CR is not supported by the spec.
    """
    block = build_manifest_block(reference, comment_prefix, comment_suffix)
    if placement == Placement.START:
        out = block + newline + text
        return StructuredEmbed(
            text=out,
            exclusion_start=0,
            exclusion_length=len((block + newline).encode("utf-8")),
        )
    # Placement.END
    start = len(text.encode("utf-8"))
    out = text + newline + block
    return StructuredEmbed(
        text=out,
        exclusion_start=start,
        exclusion_length=len((newline + block).encode("utf-8")),
    )


def extract_structured(text: str) -> StructuredExtraction:
    """Extract a manifest reference from structured text (spec A.9.5).

    Form-agnostic: the reference is whatever appears between the single pair of
    delimiters, trimmed of surrounding whitespace, so both the single-line and
    front-matter forms are handled.

    Raises :class:`StructuredError` (with the matching C2PA status code) on
    ``noManifest``, ``multipleReferences`` or ``emptyReference``.
    """
    begin_count = text.count(BEGIN_DELIMITER)
    end_count = text.count(END_DELIMITER)
    if begin_count == 0 or end_count == 0:
        raise StructuredError(
            "manifest.structuredText.noManifest",
            "No C2PA manifest block delimiters found",
        )
    if begin_count > 1 or end_count > 1:
        raise StructuredError(
            "manifest.structuredText.multipleReferences",
            "Multiple C2PA manifest blocks found",
        )
    begin = text.index(BEGIN_DELIMITER) + len(BEGIN_DELIMITER)
    end = text.index(END_DELIMITER)
    if end <= begin:
        raise StructuredError(
            "manifest.structuredText.noManifest",
            "Manifest block delimiters are out of order",
        )
    reference = text[begin:end].strip()
    if not reference:
        raise StructuredError(
            "manifest.structuredText.emptyReference",
            "Manifest reference between delimiters is empty",
        )
    return StructuredExtraction(reference=reference, manifest=decode_data_uri(reference))


_RECOMMENDED = {
    # Unstructured family (A.8). JSON and CSV have no comment/front-matter
    # syntax, so the structured method (A.9) cannot apply to them.
    "text/plain": Method.UNSTRUCTURED,
    "text/markdown": Method.UNSTRUCTURED,
    "text/csv": Method.UNSTRUCTURED,
    "application/json": Method.UNSTRUCTURED,
    # Structured family (A.9), via XML comment syntax `<!-- -->`.
    "text/xml": Method.STRUCTURED,
    "application/xml": Method.STRUCTURED,
    "application/xhtml+xml": Method.STRUCTURED,
    # Dedicated methods not implemented by this package.
    "text/html": Method.HTML,
    "image/svg+xml": Method.SVG,
}


def recommended_method(mime: str) -> Method | None:
    """Advisory recommendation of an embedding method for a media type, per the
    C2PA 2.4 spec text families. Returns ``None`` for media types with no defined
    text embedding method. Informative only -- see module docs.
    """
    if mime in _RECOMMENDED:
        return _RECOMMENDED[mime]
    if mime.startswith("text/"):
        return Method.STRUCTURED
    return None


_COMMENT_SYNTAX = {
    "text/css": ("/*", "*/"),
    "application/javascript": ("//", ""),
    "text/javascript": ("//", ""),
    "text/markdown": ("<!--", "-->"),
    "text/xml": ("<!--", "-->"),
    "application/xml": ("<!--", "-->"),
    "application/xhtml+xml": ("<!--", "-->"),
    "application/yaml": ("#", ""),
    "text/yaml": ("#", ""),
    "application/x-yaml": ("#", ""),
    "application/toml": ("#", ""),
}


def comment_syntax(mime: str) -> tuple[str, str] | None:
    """Host comment delimiters ``(prefix, suffix)`` used by the structured (A.9)
    method for a media type, so the embedded armour block stays valid host
    syntax. Returns ``None`` for media types with no comment convention (e.g.
    ``application/json``, ``text/plain``, ``text/csv``) -- use the unstructured
    (A.8) method for those.

    The delimiters are each language's own comment syntax (e.g. ``("/*", "*/")``
    for CSS); pass them to :func:`embed_structured` / :func:`build_manifest_block`.
    Distinct from :func:`recommended_method`, which advises *which* method to use.
    """
    return _COMMENT_SYNTAX.get(mime)


__all__ = [
    "BEGIN_DELIMITER",
    "END_DELIMITER",
    "DATA_URI_PREFIX",
    "Placement",
    "Method",
    "StructuredEmbed",
    "StructuredExtraction",
    "StructuredError",
    "encode_data_uri",
    "decode_data_uri",
    "build_manifest_block",
    "build_manifest_block_multiline",
    "embed_structured",
    "extract_structured",
    "recommended_method",
    "comment_syntax",
]
