# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2026-05-29

### Changed (breaking)
- **Go module is now v2**: the module path is `github.com/encypherai/c2pa-text/go/v2`.
  Update imports to `github.com/encypherai/c2pa-text/go/v2/c2pa_text` and install with
  `go get github.com/encypherai/c2pa-text/go/v2@v2.0.0`. (Required by Go's semantic
  import versioning for major versions >= 2.)
- Adopts the corrected TypeScript packaging from 1.1.1 (single CommonJS entry; no
  phantom ESM `import`/`require` exports). Consumers relying on the removed ESM paths
  must use the CommonJS entry.
- Supersedes the internal, never-released 1.2.0 / 1.3.0 version bumps; this is the
  first release of the structured (A.9) and HTML (A.7) pipelines.

### Added
- **Structured text embedding pipeline (C2PA 2.4 Appendix A.9)** in all SDKs
  (Python, TypeScript, Rust, Go): associate a C2PA Manifest Store with source
  code, config (YAML/TOML), Markdown, XML and similar comment/front-matter
  formats via an ASCII Armour-style block
  (`-----BEGIN C2PA MANIFEST----- <ref> -----END C2PA MANIFEST-----`) carrying a
  URL or `data:application/c2pa;base64,...` reference.
  - `embed_structured()` / `extract_structured()` with `c2pa.hash.data`
    exclusion-range computation (A.9.4) and the `manifest.structuredText.*`
    failure codes (A.9.5).
  - `build_manifest_block()` (single-line) and `build_manifest_block_multiline()`
    (front matter) block builders.
  - `encode_data_uri()` / `decode_data_uri()` for inline manifest references.
  - `recommended_method()` advisory media-type → pipeline mapping (informative;
    neither pipeline is restricted to a fixed set of media types).
- **HTML embedding pipeline (C2PA 2.4 Appendix A.7)** in all SDKs: associate a
  Manifest Store with an HTML document via an inline
  `<script type="application/c2pa">` element or a `<link rel="c2pa-manifest">`
  reference in the `<head>`.
  - `embed_html_inline()` (with `c2pa.hash.data` exclusion covering the script
    element, A.7.1.3), `embed_html_reference()`, `extract_html()`,
    `build_html_script()` / `build_html_link()`.
  - `manifest.html.multipleManifests` failure code (A.7.1).
- **Shared golden fixtures** (`golden/`): `vectors.json` consumed by all four
  test suites for byte-for-byte cross-language parity, real embedded sample
  files, a binary sample Manifest Store, and an illustrative crJSON fixture.

### Notes
- The structured and unstructured (Appendix A.8) pipelines are independent and
  format-agnostic; the implementer chooses which to use for a given asset.
- `image/svg+xml` (Appendix A.3.3) has its own dedicated embedding method and is
  out of scope; SVG/XML may alternatively use the structured pipeline.
- All four implementations produce byte-identical output, enforced by the shared
  golden vectors and per-language literal-assertion unit tests.

### Tooling
- CI: added a cross-language parity job that regenerates `golden/` from the
  Python reference and fails on drift, enforcing byte-identical output across all
  four SDKs on every pull request.
- Added `tools/test-all.sh` (run all four suites + golden drift locally) and an
  opt-in `pre-push` git hook via `tools/install-hooks.sh` (guarded so it does
  not hijack hooks when c2pa-text is vendored in a monorepo).
- Rust: reformatted `validator.rs` to current stable `rustfmt` (1.9.0 / rustc
  1.96.0) so `cargo fmt --check` passes in CI. Formatting only.
- Excluded `golden/` from the `trailing-whitespace`/`end-of-file-fixer`
  pre-commit hooks (byte-exact fixtures).

### Docs
- READMEs (root + Python/TypeScript/Rust) document all three embedding pipelines
  with an "Embedding methods" overview and usage for the structured and HTML
  methods; added a TypeScript package README.

## [1.1.0] - 2026-04-08

### Added
- All SDKs (Python, TypeScript, Rust, Go): `encode_wrapper_padded()` produces
  wrappers of exact, content-independent UTF-8 byte length using a 3a+4b
  gap-filling algorithm with variation selector padding characters.
- All SDKs: `worst_case_wrapper_byte_length()` computes the deterministic upper
  bound for a given manifest byte count (`3 + (13 + M) * 4 + 6`).
- Python: comprehensive test suite for deterministic padding covering formula
  correctness, round-trip decodability, edge cases, and backward compatibility.

### Notes
- Deterministic padding breaks the hash-avalanche circularity in C2PA hard
  binding computation, enabling single-pass manifest embedding without iterative
  convergence loops.
- Decoders remain backward-compatible: the `manifestLength` header field tells
  decoders where the real manifest ends, so padding is ignored.

## [1.0.3] - 2026-01-26

### Changed
- Python: wrapper exclusion offsets are now returned as NFC UTF-8 byte offsets and clean text exclusion uses byte offsets.

### Tests
- Python: added regression coverage for NFC UTF-8 byte offset semantics.

## [1.0.0-preview.1] - 2025-12-05

### Changed
- Version bumped to preview for C2PA working group review
- Updated installation instructions for GitHub-based preview installation
- Added preview status badge to README

### Notes
- API is stable and ready for review
- Not yet published to package registries (PyPI, npm, crates.io)
- Install directly from GitHub during preview period

---

## [1.0.0] - 2025-11-25 (Unreleased)

### Added

- **Core Embedding/Extraction**: Full implementation of C2PA Text Manifest Wrapper specification
  - `embed_manifest()` - Embed C2PA JUMBF manifests into UTF-8 text
  - `extract_manifest()` - Extract manifests from watermarked text
  - Unicode Variation Selector encoding (U+FE00..U+FE0F, U+E0100..U+E01EF)
  - NFC normalization for consistent text handling

- **Structural Validation**: Pre-embedding validation to catch issues early
  - `validate_manifest()` - Validate JUMBF structure before embedding
  - `validate_jumbf_structure()` - Detailed JUMBF box validation with strict mode
  - `validate_wrapper_bytes()` - Validate pre-encoded wrapper bytes
  - C2PA-compliant validation codes (e.g., `manifest.text.corruptedWrapper`)

- **Multi-Language Support**:
  - Python (PyPI: `c2pa-text`)
  - TypeScript/JavaScript (npm: `c2pa-text`)
  - Rust (crates.io: `c2pa-text`)
  - Go (`github.com/encypherai/c2pa-text/go`)

- **Documentation**:
  - Comprehensive README with usage examples for all languages
  - Validation API documentation
  - MIT License

### Technical Details

- Implements `C2PATextManifestWrapper` per `Manifests_Text.adoc` specification
- Magic bytes: `C2PATXT\0` (0x4332504154585400)
- Header structure: Big-endian `!8sBI` (magic + version + length)
- ZWNBSP prefix (U+FEFF) for wrapper detection
