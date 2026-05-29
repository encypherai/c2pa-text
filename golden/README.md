# Golden fixtures

Shared, language-agnostic test fixtures that lock the **byte-level wire format**
of the two `c2pa-text` embedding pipelines defined by the C2PA Technical
Specification 2.4:

- **Unstructured** — Unicode Variation Selector `C2PATextManifestWrapper`
  (Appendix A.8).
- **Structured** — ASCII Armour-style manifest block inside a host comment or
  front matter, carrying a manifest reference (URL or `data:` URI) (Appendix A.9).

Every language implementation (Rust, Python, TypeScript, Go) has a test that
loads `vectors.json` and asserts **byte-for-byte** reproduction of these values.
Because all four assert against the same goldens, passing them proves
cross-language parity (identical embeddings from identical inputs).

## Files

| File | Purpose |
|------|---------|
| `vectors.json` | Canonical inputs + expected outputs (hex/strings) for both pipelines. The single source of truth consumed by all four test suites. |
| `manifest_store.bin` | A deterministic 256-byte sample "Manifest Store" (the opaque JUMBF bytes that `c2pa-text` embeds). Bytes `0x00..0xFF`. |
| `samples/note.txt` | Real **unstructured** embed (A.8): a plain-text note with an invisible variation-selector wrapper appended. |
| `samples/script.py` | Real **structured** embed (A.9): single-line `#` comment with an external manifest URL, at start of file. |
| `samples/document.md` | Real **structured** embed (A.9): Markdown `<!-- -->` comment with an inline `data:application/c2pa;base64,...` reference. |
| `sample.crjson` | **Illustrative only.** The human-readable Content Credentials JSON (crJSON) view of the kind of manifest a store represents. Hash/signature are placeholders; it is not signed or validated. `c2pa-text` embeds the binary store, not crJSON. |
| `generate.py` | Regenerates `vectors.json`, `manifest_store.bin`, and `samples/` from the Python reference implementation. |

## Regenerating

```sh
PYTHONPATH=python/src python3 golden/generate.py
```

`vectors.json` is generated from the Python reference implementation, which the
parity tests prove byte-identical to the Rust implementation. Each language
suite additionally carries hand-written literal assertions, so the goldens are
cross-checked independently of the generator.

## `vectors.json` schema

- `data_uri[]` — `{ name, manifest_hex, expected_uri }` for `encodeDataUri`.
- `structured_block[]` — `{ name, reference, comment_prefix, comment_suffix, expected_block }` for the single-line block builder.
- `structured_multiline[]` — `{ name, reference, newline, expected_block }` for the front-matter block builder.
- `structured_embed[]` — `{ name, text, reference, comment_prefix, comment_suffix, placement, newline, expected_text_hex, exclusion_start, exclusion_length }` for `embedStructured` (hex is UTF-8 bytes).
- `unstructured_embed[]` — `{ name, text, manifest_hex, expected_embed_hex }` for `embedManifest`.
