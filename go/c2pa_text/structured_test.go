package c2pa_text

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestStructuredDataURIRoundTrip(t *testing.T) {
	data := []byte{0x01, 0x02, 0x03, 0xFF, 0x00}
	uri := EncodeDataURI(data)
	if uri != "data:application/c2pa;base64,AQID/wA=" {
		t.Fatalf("unexpected uri: %q", uri)
	}
	decoded, ok := DecodeDataURI(uri)
	if !ok || !bytes.Equal(decoded, data) {
		t.Fatalf("round trip failed: ok=%v decoded=%x", ok, decoded)
	}
	if _, ok := DecodeDataURI("https://example.com/m.c2pa"); ok {
		t.Fatalf("non-data URI should not decode")
	}
	if _, ok := DecodeDataURI("data:application/c2pa;base64,!!!!"); ok {
		t.Fatalf("invalid base64 should not decode")
	}
}

func TestStructuredBlockFormat(t *testing.T) {
	if got := BuildManifestBlock("https://x/m.c2pa", "#", ""); got != "# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----" {
		t.Fatalf("line comment block: %q", got)
	}
	if got := BuildManifestBlock("https://x/m.c2pa", "<!--", "-->"); got != "<!-- -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST----- -->" {
		t.Fatalf("xml comment block: %q", got)
	}
	if got := BuildManifestBlockMultiline("https://x/m.c2pa", "\n"); got != "-----BEGIN C2PA MANIFEST-----\nhttps://x/m.c2pa\n-----END C2PA MANIFEST-----" {
		t.Fatalf("multiline block: %q", got)
	}
}

func TestEmbedStructuredStart(t *testing.T) {
	block := "# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----"
	r := EmbedStructured("body line 1\nbody line 2\n", "https://x/m.c2pa", "#", "", PlacementStart, "\n")
	if r.Text != block+"\nbody line 1\nbody line 2\n" {
		t.Fatalf("text: %q", r.Text)
	}
	if r.ExclusionStart != 0 || r.ExclusionLength != len(block)+1 {
		t.Fatalf("exclusion: start=%d len=%d", r.ExclusionStart, r.ExclusionLength)
	}
	excluded := r.Text[r.ExclusionStart : r.ExclusionStart+r.ExclusionLength]
	if excluded != block+"\n" {
		t.Fatalf("excluded region: %q", excluded)
	}
	x, err := ExtractStructured(r.Text)
	if err != nil || x.Reference != "https://x/m.c2pa" || x.Manifest != nil {
		t.Fatalf("extract: %+v err=%v", x, err)
	}
}

func TestEmbedStructuredEnd(t *testing.T) {
	text := "#!/usr/bin/env python\nprint('hi')\n"
	block := "# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----"
	r := EmbedStructured(text, "https://x/m.c2pa", "#", "", PlacementEnd, "\n")
	if r.Text != text+"\n"+block {
		t.Fatalf("text: %q", r.Text)
	}
	if r.ExclusionStart != len(text) || r.ExclusionLength != 1+len(block) {
		t.Fatalf("exclusion: start=%d len=%d", r.ExclusionStart, r.ExclusionLength)
	}
}

func TestEmbedExtractDataURI(t *testing.T) {
	manifest := []byte{0xDE, 0xAD, 0xBE, 0xEF}
	uri := EncodeDataURI(manifest)
	r := EmbedStructured("doc\n", uri, "//", "", PlacementStart, "\n")
	x, err := ExtractStructured(r.Text)
	if err != nil {
		t.Fatalf("extract err: %v", err)
	}
	if x.Reference != uri || !bytes.Equal(x.Manifest, manifest) {
		t.Fatalf("extract: %+v", x)
	}
}

func TestExtractErrors(t *testing.T) {
	cases := []struct {
		text string
		want error
	}{
		{"nothing here", ErrNoManifest},
		{"# " + BeginDelimiter + " https://x", ErrNoManifest},
		{"# " + BeginDelimiter + "   " + EndDelimiter, ErrEmptyReference},
		{"# " + BeginDelimiter + " a " + EndDelimiter + "\n# " + BeginDelimiter + " b " + EndDelimiter, ErrMultipleReferences},
	}
	for _, c := range cases {
		_, err := ExtractStructured(c.text)
		if !errors.Is(err, c.want) {
			t.Fatalf("text %q: want %v got %v", c.text, c.want, err)
		}
	}
}

func TestFrontMatterExtract(t *testing.T) {
	doc := "---\n-----BEGIN C2PA MANIFEST-----\nhttps://x/m.c2pa\n-----END C2PA MANIFEST-----\ntitle: Doc\n---\nbody\n"
	x, err := ExtractStructured(doc)
	if err != nil || x.Reference != "https://x/m.c2pa" {
		t.Fatalf("front matter: %+v err=%v", x, err)
	}
}

func TestRecommendedMethod(t *testing.T) {
	cases := map[string]Method{
		"text/plain":            MethodUnstructured,
		"text/markdown":         MethodUnstructured,
		"text/csv":              MethodUnstructured,
		"application/json":      MethodUnstructured,
		"application/xml":       MethodStructured,
		"text/xml":              MethodStructured,
		"application/xhtml+xml": MethodStructured,
		"text/x-python":         MethodStructured,
		"text/html":             MethodHTML,
		"image/svg+xml":         MethodSVG,
		"image/jpeg":            MethodNone,
	}
	for mime, want := range cases {
		if got := RecommendedMethod(mime); got != want {
			t.Fatalf("mime %q: want %v got %v", mime, want, got)
		}
	}
}

func TestCommentSyntax(t *testing.T) {
	type cs struct {
		prefix, suffix string
		ok             bool
	}
	cases := map[string]cs{
		"text/css":               {"/*", "*/", true},
		"application/javascript": {"//", "", true},
		"application/xml":        {"<!--", "-->", true},
		"text/markdown":          {"<!--", "-->", true},
		"application/yaml":       {"#", "", true},
		"application/toml":       {"#", "", true},
		"application/json":       {"", "", false},
		"text/plain":             {"", "", false},
		"text/csv":               {"", "", false},
		"image/jpeg":             {"", "", false},
	}
	for mime, want := range cases {
		p, s, ok := CommentSyntax(mime)
		if p != want.prefix || s != want.suffix || ok != want.ok {
			t.Fatalf("mime %q: want (%q,%q,%v) got (%q,%q,%v)", mime, want.prefix, want.suffix, want.ok, p, s, ok)
		}
	}
	// Resolved delimiters compose into a valid host comment.
	p, s, _ := CommentSyntax("text/css")
	got := BuildManifestBlock("data:application/c2pa;base64,AA==", p, s)
	want := "/* -----BEGIN C2PA MANIFEST----- data:application/c2pa;base64,AA== -----END C2PA MANIFEST----- */"
	if got != want {
		t.Fatalf("block: want %q got %q", want, got)
	}
}

// --- Golden vectors (cross-language parity) ---

type goldenVectors struct {
	DataURI []struct {
		Name        string `json:"name"`
		ManifestHex string `json:"manifest_hex"`
		ExpectedURI string `json:"expected_uri"`
	} `json:"data_uri"`
	StructuredBlock []struct {
		Name          string `json:"name"`
		Reference     string `json:"reference"`
		CommentPrefix string `json:"comment_prefix"`
		CommentSuffix string `json:"comment_suffix"`
		ExpectedBlock string `json:"expected_block"`
	} `json:"structured_block"`
	StructuredMultiline []struct {
		Name          string `json:"name"`
		Reference     string `json:"reference"`
		Newline       string `json:"newline"`
		ExpectedBlock string `json:"expected_block"`
	} `json:"structured_multiline"`
	StructuredEmbed []struct {
		Name            string `json:"name"`
		Text            string `json:"text"`
		Reference       string `json:"reference"`
		CommentPrefix   string `json:"comment_prefix"`
		CommentSuffix   string `json:"comment_suffix"`
		Placement       string `json:"placement"`
		Newline         string `json:"newline"`
		ExpectedTextHex string `json:"expected_text_hex"`
		ExclusionStart  int    `json:"exclusion_start"`
		ExclusionLength int    `json:"exclusion_length"`
	} `json:"structured_embed"`
	UnstructuredEmbed []struct {
		Name             string `json:"name"`
		Text             string `json:"text"`
		ManifestHex      string `json:"manifest_hex"`
		ExpectedEmbedHex string `json:"expected_embed_hex"`
	} `json:"unstructured_embed"`
}

func loadGolden(t *testing.T) goldenVectors {
	t.Helper()
	path := filepath.Join("..", "..", "golden", "vectors.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read golden: %v", err)
	}
	var v goldenVectors
	if err := json.Unmarshal(raw, &v); err != nil {
		t.Fatalf("parse golden: %v", err)
	}
	return v
}

func TestGoldenVectors(t *testing.T) {
	g := loadGolden(t)

	for _, v := range g.DataURI {
		manifest, _ := hex.DecodeString(v.ManifestHex)
		if got := EncodeDataURI(manifest); got != v.ExpectedURI {
			t.Fatalf("data_uri %s: got %q", v.Name, got)
		}
		decoded, ok := DecodeDataURI(v.ExpectedURI)
		if !ok || hex.EncodeToString(decoded) != v.ManifestHex {
			t.Fatalf("data_uri %s decode mismatch", v.Name)
		}
	}

	for _, v := range g.StructuredBlock {
		if got := BuildManifestBlock(v.Reference, v.CommentPrefix, v.CommentSuffix); got != v.ExpectedBlock {
			t.Fatalf("structured_block %s: got %q", v.Name, got)
		}
	}

	for _, v := range g.StructuredMultiline {
		if got := BuildManifestBlockMultiline(v.Reference, v.Newline); got != v.ExpectedBlock {
			t.Fatalf("structured_multiline %s: got %q", v.Name, got)
		}
	}

	for _, v := range g.StructuredEmbed {
		placement := PlacementStart
		if v.Placement == "end" {
			placement = PlacementEnd
		}
		r := EmbedStructured(v.Text, v.Reference, v.CommentPrefix, v.CommentSuffix, placement, v.Newline)
		if hex.EncodeToString([]byte(r.Text)) != v.ExpectedTextHex {
			t.Fatalf("structured_embed %s: text hex mismatch", v.Name)
		}
		if r.ExclusionStart != v.ExclusionStart || r.ExclusionLength != v.ExclusionLength {
			t.Fatalf("structured_embed %s: exclusion start=%d len=%d", v.Name, r.ExclusionStart, r.ExclusionLength)
		}
		x, err := ExtractStructured(r.Text)
		if err != nil || x.Reference != v.Reference {
			t.Fatalf("structured_embed %s: extract %+v err=%v", v.Name, x, err)
		}
	}

	for _, v := range g.UnstructuredEmbed {
		manifest, _ := hex.DecodeString(v.ManifestHex)
		embedded := EmbedManifest(v.Text, manifest)
		if hex.EncodeToString([]byte(embedded)) != v.ExpectedEmbedHex {
			t.Fatalf("unstructured_embed %s: embed hex mismatch", v.Name)
		}
		extracted, _, _, _, err := ExtractManifest(embedded)
		if err != nil || hex.EncodeToString(extracted) != v.ManifestHex {
			t.Fatalf("unstructured_embed %s: extract mismatch err=%v", v.Name, err)
		}
	}
}
