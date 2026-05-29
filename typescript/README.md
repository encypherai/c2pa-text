<div align="center">
  <a href="https://encypher.com">
    <img src="https://encypher.com/encypher_full_nobg.png" alt="Encypher Corporation Logo" width="200">
  </a>

  # c2pa-text

  **A Reference Implementation for C2PA Text Embedding**

  [![Status](https://img.shields.io/badge/Status-Stable-brightgreen)](https://github.com/encypherai/c2pa-text)
  [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
  [![C2PA Compliant](https://img.shields.io/badge/C2PA-Compliant-blue)](https://c2pa.org)
  [![Python](https://img.shields.io/pypi/v/c2pa-text?color=3776AB&logo=python&logoColor=white)](https://pypi.org/project/c2pa-text/)
  [![NPM](https://img.shields.io/npm/v/c2pa-text?color=CB3837&logo=npm&logoColor=white)](https://www.npmjs.com/package/c2pa-text)
  [![Rust](https://img.shields.io/crates/v/c2pa-text?color=dea584&logo=rust&logoColor=white)](https://crates.io/crates/c2pa-text)
  [![Go](https://img.shields.io/badge/Go-Reference-00ADD8?logo=go&logoColor=white)](https://pkg.go.dev/github.com/encypherai/c2pa-text/go)
</div>


---

This library embeds and extracts [C2PA](https://c2pa.org) manifests in **text** assets, implementing all three text embedding methods defined by the C2PA 2.4 specification: **unstructured** (invisible Unicode Variation Selectors, Appendix A.8), **structured** (comment / front-matter ASCII Armour blocks, A.9), and **HTML** (`<script>` / `<link>`, A.7).

## Overview

C2PA manifests are typically embedded in binary files (JPEG, PNG, MP4). For plain text, this library implements a standard wrapper structure (`C2PATextManifestWrapper`) that encodes the binary C2PA Manifest Store (JUMBF) into invisible characters that persist through copy-paste operations.

This repository contains implementations for:
- **Python**: For backend services and data processing.
- **TypeScript**: For browser extensions, web apps, and Node.js.
- **Rust**: For high-performance CLI tools and Wasm.
- **Go**: For backend microservices.

## Embedding methods

C2PA 2.4 defines three ways to associate a Manifest Store with a text asset. This
library implements all three as independent, format-agnostic pipelines — the
implementer chooses which fits a given asset:

| Method | Spec | Mechanism | Typical assets |
|--------|------|-----------|----------------|
| **Unstructured** | A.8 | Invisible Unicode Variation Selector wrapper appended to the text | `text/plain`, `text/markdown`, copy-paste-safe snippets |
| **Structured** | A.9 | ASCII Armour block (`-----BEGIN C2PA MANIFEST----- … -----END C2PA MANIFEST-----`) inside a host comment or front matter, carrying a URL or `data:` URI | source code, YAML/TOML, Markdown, XML |
| **HTML** | A.7 | `<script type="application/c2pa">` (inline) or `<link rel="c2pa-manifest">` (external) in the `<head>` | `text/html` |

`recommended_method(mime)` returns an advisory pick per media type, but it is
informative only — any UTF-8 text asset may use any pipeline.

## Unstructured wrapper (Appendix A.8)

The wrapper structure is defined as:

```
Container Type: C2PATextManifestWrapper
Magic: "C2PATXT\0" (0x4332504154585400)
Version: 1
Encoding: Unicode Variation Selectors (U+FE00..U+FE0F, U+E0100..U+E01EF)
Placement: End of text, prefixed with ZWNBSP (U+FEFF)
```

## Maintenance & Support

This library is the official reference implementation maintained by **Encypher** (encypher.com), authors of the C2PA Text Specification and active contributors to the C2PA standard.

While this library is free and permissively licensed (MIT), Encypher offers an **Enterprise API** for:
- Managing cryptographic keys at scale (HSM)
- Analytics and tracking for embedded content
- Automated verification and revocation
- Content production workflows

[Learn more about Encypher Enterprise](https://encypher.com)

## Installation
```bash
# Python
pip install c2pa-text

# TypeScript
npm install c2pa-text

# Rust
cargo add c2pa-text

# Go
go get github.com/encypherai/c2pa-text/go/v2@v2.0.0
```

## Generating Manifests

This library handles the **embedding layer** (text steganography). To generate the valid C2PA JUMBF manifest bytes (`manifest_bytes`), you have two options:

### 1. Use Encypher API (Recommended)
The [Encypher Enterprise API](https://encypher.com) automatically handles key management, signing, and manifest generation. It returns the fully signed JUMBF bytes or the final watermarked text directly.

### 2. Use C2PA Tooling
You can generate raw JUMBF manifests using standard C2PA tools (like `c2pa-rs` or `c2patool`) and pass the binary output to this library.

## Usage (Python)

```python
from c2pa_text import embed_manifest, extract_manifest

# 1. You have a binary C2PA manifest (JUMBF)
manifest_bytes = b"..."

# 2. Embed it into text
text = "Hello World"
watermarked_text = embed_manifest(text, manifest_bytes)

# 3. Extract it back
extracted_bytes, clean_text = extract_manifest(watermarked_text)
```

### Validation (Python)

Validate an entire text document or individual manifests before embedding:

```python
from c2pa_text import validate_text, validate_manifest

# Validate a text document (scans for wrappers, checks structure)
result = validate_text(signed_text)
if result.valid:
    print("Document is well-formed")
else:
    for issue in result.issues:
        print(f"  [{issue.code}] {issue.message}")
    # Example output:
    #   [manifest.text.multipleWrappers] Multiple C2PA wrappers found (2)

# Validate manifest bytes before embedding
result = validate_manifest(manifest_bytes)
if result.valid:
    watermarked = embed_manifest(text, manifest_bytes)
```

Available validation functions:
- `validate_text(text)` - Validate an entire text document (scans for wrappers, checks each structurally)
- `validate_manifest(bytes)` - Validate JUMBF structure before embedding
- `validate_jumbf_structure(bytes, strict=True)` - Strict C2PA compliance checks
- `validate_wrapper_bytes(bytes)` - Validate pre-encoded wrapper bytes

Validation codes follow the C2PA conformance rubric vocabulary:
- `manifest.text.corruptedWrapper` - Invalid JUMBF structure in wrapper
- `manifest.text.multipleWrappers` - More than one wrapper found
- `manifest.text.invalidMagic` - Bad C2PA magic bytes
- `manifest.text.unsupportedVersion` - Unrecognized wrapper version
- `manifest.text.lengthMismatch` - Declared length exceeds actual JUMBF data
- `manifest.text.emptyManifest` - Zero-length JUMBF payload

## Usage (TypeScript)

```typescript
import { embedManifest, extractManifest, validateManifest, validateText } from 'c2pa-text';

// 1. You have a binary C2PA manifest (JUMBF) as a Uint8Array
const manifestBytes = new Uint8Array([/* ... */]);

// 2. Validate before embedding (optional but recommended)
const validation = validateManifest(manifestBytes);
if (!validation.valid) {
  console.error(validation.issues);
  throw new Error('Invalid manifest');
}

// 3. Embed it into text
const text = "Hello World";
const watermarkedText = embedManifest(text, manifestBytes);

// 4. Extract it back
const result = extractManifest(watermarkedText);
if (result) {
  console.log(result.manifest);   // Uint8Array
  console.log(result.cleanText);  // "Hello World"
}

// 5. Validate an existing signed document
const docResult = validateText(watermarkedText);
console.log(docResult.valid);     // true
```

## Usage (Rust)

```rust
use c2pa_text::{embed_manifest, extract_manifest, validate_manifest, validate_text};

// 1. Binary manifest
let manifest_bytes = b"...";

// 2. Validate before embedding (optional but recommended)
let validation = validate_manifest(manifest_bytes, true, false);
if !validation.valid {
    eprintln!("{}", validation);
    return Err("Invalid manifest");
}

// 3. Embed
let text = "Hello World";
let watermarked = embed_manifest(text, manifest_bytes);

// 4. Extract
if let Ok(result) = extract_manifest(&watermarked) {
    if let Some(bytes) = result.manifest {
        println!("Extracted {} bytes", bytes.len());
    }
}

// 5. Validate an existing signed document
let doc_result = validate_text(&watermarked);
assert!(doc_result.valid);
```

## Usage (Go)

```go
import "github.com/encypherai/c2pa-text/go/v2/c2pa_text"

// 1. Binary manifest
manifestBytes := []byte("...")

// 2. Validate before embedding (optional but recommended)
validation := c2pa_text.ValidateManifest(manifestBytes, true, false)
if !validation.Valid {
    fmt.Println(validation)
    return errors.New("invalid manifest")
}

// 3. Embed
text := "Hello World"
watermarked := c2pa_text.EmbedManifest(text, manifestBytes)

// 4. Extract
extractedBytes, cleanText, _, _, err := c2pa_text.ExtractManifest(watermarked)

// 5. Validate an existing signed document
docResult := c2pa_text.ValidateText(watermarked)
fmt.Println(docResult.Valid)  // true
```

## Structured Text (Appendix A.9)

Embed a manifest *reference* — an external URL, or an inline `data:` URI — inside a
host comment or front matter. The embed call returns the text plus the
`c2pa.hash.data` exclusion range (byte offsets) to hard-bind it.

```python
from c2pa_text import embed_structured, extract_structured, encode_data_uri, Placement

# Reference an external manifest from a Python source file (comment prefix "#")
r = embed_structured(source_code, "https://example.com/m.c2pa", "#")
print(r.text)                                 # source with the manifest block
print(r.exclusion_start, r.exclusion_length)  # c2pa.hash.data exclusion (bytes)

# Or embed the manifest inline as a data: URI (e.g. a JS file, "//" comment)
r = embed_structured(source_code, encode_data_uri(manifest_bytes), "//")

# Extract (handles both single-line comment and front-matter forms)
ex = extract_structured(r.text)
print(ex.reference)   # the URL or data: URI
print(ex.manifest)    # decoded bytes if a data: URI, else None
```

TypeScript (same API, camelCase): `embedStructured` / `extractStructured` /
`encodeDataUri`. Rust: `c2pa_text::structured::{embed_structured, extract_structured, …}`.
Go: `c2pa_text.EmbedStructured` / `ExtractStructured`. Comment styles include
`#`, `//`, `--`, `/* */`, `<!-- -->`; place the block with `Placement.START`
(default) or `Placement.END` (when the first line is reserved, e.g. a shebang or
`<?xml ?>` declaration). Validation failure codes (spec A.9.5):
`manifest.structuredText.noManifest`, `…multipleReferences`, `…emptyReference`.

## HTML (Appendix A.7)

Associate a manifest with an HTML document via an inline `<script>` element or an
external `<link>` reference, placed in the `<head>`.

```python
from c2pa_text import embed_html_inline, embed_html_reference, extract_html

# Inline: <script type="application/c2pa">base64…</script>
r = embed_html_inline(html, manifest_bytes)
print(r.exclusion_start, r.exclusion_length)  # exclusion over the <script> element

# External (preferred): <link rel="c2pa-manifest" href="…">
html_out = embed_html_reference(html, "https://example.com/manifest.c2pa")

ex = extract_html(r.text)   # None if the document has no C2PA association
print(ex.method)            # "inline" or "reference"
print(ex.manifest, ex.reference)
```

TypeScript: `embedHtmlInline` / `embedHtmlReference` / `extractHtml`. Rust:
`c2pa_text::html::{embed_html_inline, embed_html_reference, extract_html, …}`. Go:
`c2pa_text.EmbedHTMLInline` / `EmbedHTMLReference` / `ExtractHTML`. A document
carries at most one association; encountering more is the spec A.7.1 failure code
`manifest.html.multipleManifests`.

## License

MIT
