"""Cross-language golden parity test (Python side).

Loads the shared ``golden/vectors.json`` fixtures and asserts byte-for-byte
reproduction. The Rust, TypeScript and Go suites assert against the same file,
so passing all four proves the implementations produce identical embeddings
from identical inputs.
"""

import json
from pathlib import Path

import pytest

from c2pa_text import (
    Placement,
    build_manifest_block,
    build_manifest_block_multiline,
    decode_data_uri,
    embed_manifest,
    embed_structured,
    encode_data_uri,
    extract_manifest,
    extract_structured,
)

GOLDEN = json.loads(
    (Path(__file__).resolve().parents[2] / "golden" / "vectors.json").read_text(encoding="utf-8")
)


def _hex(s: str) -> str:
    return s.encode("utf-8").hex()


@pytest.mark.parametrize("v", GOLDEN["data_uri"], ids=lambda v: v["name"])
def test_golden_data_uri(v):
    manifest = bytes.fromhex(v["manifest_hex"])
    assert encode_data_uri(manifest) == v["expected_uri"]
    assert decode_data_uri(v["expected_uri"]).hex() == v["manifest_hex"]


@pytest.mark.parametrize("v", GOLDEN["structured_block"], ids=lambda v: v["name"])
def test_golden_structured_block(v):
    assert (
        build_manifest_block(v["reference"], v["comment_prefix"], v["comment_suffix"])
        == v["expected_block"]
    )


@pytest.mark.parametrize("v", GOLDEN["structured_multiline"], ids=lambda v: v["name"])
def test_golden_structured_multiline(v):
    assert build_manifest_block_multiline(v["reference"], v["newline"]) == v["expected_block"]


@pytest.mark.parametrize("v", GOLDEN["structured_embed"], ids=lambda v: v["name"])
def test_golden_structured_embed(v):
    placement = Placement.END if v["placement"] == "end" else Placement.START
    r = embed_structured(
        v["text"],
        v["reference"],
        v["comment_prefix"],
        v["comment_suffix"],
        placement,
        v["newline"],
    )
    assert _hex(r.text) == v["expected_text_hex"]
    assert r.exclusion_start == v["exclusion_start"]
    assert r.exclusion_length == v["exclusion_length"]
    assert extract_structured(r.text).reference == v["reference"]


@pytest.mark.parametrize("v", GOLDEN["unstructured_embed"], ids=lambda v: v["name"])
def test_golden_unstructured_embed(v):
    manifest = bytes.fromhex(v["manifest_hex"])
    embedded = embed_manifest(v["text"], manifest)
    assert _hex(embedded) == v["expected_embed_hex"]
    extracted, _clean = extract_manifest(embedded)
    assert extracted.hex() == v["manifest_hex"]
