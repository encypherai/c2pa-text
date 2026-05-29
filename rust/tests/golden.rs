//! Cross-language golden parity test.
//!
//! Loads the shared `golden/vectors.json` fixtures and asserts byte-for-byte
//! reproduction by the Rust implementation. The Python, TypeScript and Go
//! suites assert against the same file, so passing all four proves the
//! implementations produce identical embeddings from identical inputs.

use std::fs;
use std::path::PathBuf;

use c2pa_text::html::{embed_html_inline, embed_html_reference, extract_html};
use c2pa_text::structured::{
    build_manifest_block, build_manifest_block_multiline, embed_structured, encode_data_uri,
    extract_structured, Placement,
};
use c2pa_text::{embed_manifest, extract_manifest};
use serde_json::Value;

fn golden() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("golden")
        .join("vectors.json");
    let raw = fs::read_to_string(&path).expect("read golden/vectors.json");
    serde_json::from_str(&raw).expect("parse golden/vectors.json")
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[test]
fn golden_data_uri() {
    for v in golden()["data_uri"].as_array().unwrap() {
        let manifest = unhex(v["manifest_hex"].as_str().unwrap());
        let expected = v["expected_uri"].as_str().unwrap();
        assert_eq!(encode_data_uri(&manifest), expected, "{}", v["name"]);
        let decoded = c2pa_text::structured::decode_data_uri(expected).unwrap();
        assert_eq!(hex(&decoded), v["manifest_hex"].as_str().unwrap());
    }
}

#[test]
fn golden_structured_block() {
    for v in golden()["structured_block"].as_array().unwrap() {
        let got = build_manifest_block(
            v["reference"].as_str().unwrap(),
            v["comment_prefix"].as_str().unwrap(),
            v["comment_suffix"].as_str().unwrap(),
        );
        assert_eq!(got, v["expected_block"].as_str().unwrap(), "{}", v["name"]);
    }
}

#[test]
fn golden_structured_multiline() {
    for v in golden()["structured_multiline"].as_array().unwrap() {
        let got = build_manifest_block_multiline(
            v["reference"].as_str().unwrap(),
            v["newline"].as_str().unwrap(),
        );
        assert_eq!(got, v["expected_block"].as_str().unwrap(), "{}", v["name"]);
    }
}

#[test]
fn golden_structured_embed() {
    for v in golden()["structured_embed"].as_array().unwrap() {
        let placement = if v["placement"].as_str().unwrap() == "end" {
            Placement::End
        } else {
            Placement::Start
        };
        let r = embed_structured(
            v["text"].as_str().unwrap(),
            v["reference"].as_str().unwrap(),
            v["comment_prefix"].as_str().unwrap(),
            v["comment_suffix"].as_str().unwrap(),
            placement,
            v["newline"].as_str().unwrap(),
        );
        assert_eq!(
            hex(r.text.as_bytes()),
            v["expected_text_hex"].as_str().unwrap(),
            "{}",
            v["name"]
        );
        assert_eq!(
            r.exclusion_start as u64,
            v["exclusion_start"].as_u64().unwrap()
        );
        assert_eq!(
            r.exclusion_length as u64,
            v["exclusion_length"].as_u64().unwrap()
        );
        let x = extract_structured(&r.text).unwrap();
        assert_eq!(x.reference, v["reference"].as_str().unwrap());
    }
}

#[test]
fn golden_unstructured_embed() {
    for v in golden()["unstructured_embed"].as_array().unwrap() {
        let manifest = unhex(v["manifest_hex"].as_str().unwrap());
        let embedded = embed_manifest(v["text"].as_str().unwrap(), &manifest);
        assert_eq!(
            hex(embedded.as_bytes()),
            v["expected_embed_hex"].as_str().unwrap(),
            "{}",
            v["name"]
        );
        let extracted = extract_manifest(&embedded).unwrap();
        assert_eq!(
            hex(&extracted.manifest.unwrap()),
            v["manifest_hex"].as_str().unwrap()
        );
    }
}

#[test]
fn golden_html_inline() {
    for v in golden()["html_inline"].as_array().unwrap() {
        let manifest = unhex(v["manifest_hex"].as_str().unwrap());
        let r = embed_html_inline(
            v["html"].as_str().unwrap(),
            &manifest,
            v["newline"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            hex(r.html.as_bytes()),
            v["expected_html_hex"].as_str().unwrap(),
            "{}",
            v["name"]
        );
        assert_eq!(
            r.exclusion_start as u64,
            v["exclusion_start"].as_u64().unwrap()
        );
        assert_eq!(
            r.exclusion_length as u64,
            v["exclusion_length"].as_u64().unwrap()
        );
        let x = extract_html(&r.html).unwrap().unwrap();
        assert_eq!(
            hex(&x.manifest.unwrap()),
            v["manifest_hex"].as_str().unwrap()
        );
    }
}

#[test]
fn golden_html_reference() {
    for v in golden()["html_reference"].as_array().unwrap() {
        let html = embed_html_reference(
            v["html"].as_str().unwrap(),
            v["url"].as_str().unwrap(),
            v["newline"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            hex(html.as_bytes()),
            v["expected_html_hex"].as_str().unwrap(),
            "{}",
            v["name"]
        );
        let x = extract_html(&html).unwrap().unwrap();
        assert_eq!(x.reference.as_deref(), Some(v["url"].as_str().unwrap()));
    }
}
