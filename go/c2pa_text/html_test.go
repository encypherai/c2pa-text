package c2pa_text

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const htmlDoc = "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n" +
	"<meta charset=\"utf-8\">\n<title>Example</title>\n</head>\n" +
	"<body>\n<p>Content here.</p>\n</body>\n</html>\n"

func TestHTMLBuilders(t *testing.T) {
	if got := BuildHTMLScript([]byte{0xDE, 0xAD, 0xBE, 0xEF}); got != `<script type="application/c2pa">3q2+7w==</script>` {
		t.Fatalf("script: %q", got)
	}
	if got := BuildHTMLLink("https://x/m.c2pa"); got != `<link rel="c2pa-manifest" href="https://x/m.c2pa" type="application/c2pa">` {
		t.Fatalf("link: %q", got)
	}
}

func TestHTMLInline(t *testing.T) {
	manifest := []byte{0xDE, 0xAD, 0xBE, 0xEF}
	r, err := EmbedHTMLInline(htmlDoc, manifest, "\n")
	if err != nil {
		t.Fatalf("embed err: %v", err)
	}
	element := `<script type="application/c2pa">3q2+7w==</script>`
	excluded := r.HTML[r.ExclusionStart : r.ExclusionStart+r.ExclusionLength]
	if excluded != element {
		t.Fatalf("excluded region: %q", excluded)
	}
	if !strings.Contains(r.HTML, element+"\n</head>") {
		t.Fatalf("element not before </head>: %q", r.HTML)
	}
	x, err := ExtractHTML(r.HTML)
	if err != nil || x == nil || x.Method != "inline" || !bytes.Equal(x.Manifest, manifest) {
		t.Fatalf("extract: %+v err=%v", x, err)
	}
}

func TestHTMLInlineNoHead(t *testing.T) {
	_, err := EmbedHTMLInline("<p>no head</p>", []byte{0x00}, "\n")
	if !errors.Is(err, ErrHTMLNoHead) {
		t.Fatalf("want ErrHTMLNoHead, got %v", err)
	}
}

func TestHTMLReference(t *testing.T) {
	url := "https://fabrikam.com/manifest.c2pa"
	html, err := EmbedHTMLReference(htmlDoc, url, "\n")
	if err != nil {
		t.Fatalf("embed err: %v", err)
	}
	x, err := ExtractHTML(html)
	if err != nil || x == nil || x.Method != "reference" || x.Reference != url || x.Manifest != nil {
		t.Fatalf("extract: %+v err=%v", x, err)
	}
}

func TestHTMLExtractNone(t *testing.T) {
	x, err := ExtractHTML(htmlDoc)
	if err != nil || x != nil {
		t.Fatalf("want (nil,nil), got %+v err=%v", x, err)
	}
}

func TestHTMLMultipleManifests(t *testing.T) {
	r, _ := EmbedHTMLInline(htmlDoc, []byte{0x00}, "\n")
	doubled, _ := EmbedHTMLReference(r.HTML, "https://x/m.c2pa", "\n")
	_, err := ExtractHTML(doubled)
	if !errors.Is(err, ErrHTMLMultipleManifests) {
		t.Fatalf("want ErrHTMLMultipleManifests, got %v", err)
	}
}

func TestHTMLGoldenVectors(t *testing.T) {
	raw, err := os.ReadFile(filepath.Join("..", "..", "golden", "vectors.json"))
	if err != nil {
		t.Fatalf("read golden: %v", err)
	}
	var g struct {
		HTMLInline []struct {
			Name            string `json:"name"`
			HTML            string `json:"html"`
			ManifestHex     string `json:"manifest_hex"`
			Newline         string `json:"newline"`
			ExpectedHTMLHex string `json:"expected_html_hex"`
			ExclusionStart  int    `json:"exclusion_start"`
			ExclusionLength int    `json:"exclusion_length"`
		} `json:"html_inline"`
		HTMLReference []struct {
			Name            string `json:"name"`
			HTML            string `json:"html"`
			URL             string `json:"url"`
			Newline         string `json:"newline"`
			ExpectedHTMLHex string `json:"expected_html_hex"`
		} `json:"html_reference"`
	}
	if err := json.Unmarshal(raw, &g); err != nil {
		t.Fatalf("parse golden: %v", err)
	}

	for _, v := range g.HTMLInline {
		manifest, _ := hex.DecodeString(v.ManifestHex)
		r, err := EmbedHTMLInline(v.HTML, manifest, v.Newline)
		if err != nil {
			t.Fatalf("html_inline %s: %v", v.Name, err)
		}
		if hex.EncodeToString([]byte(r.HTML)) != v.ExpectedHTMLHex {
			t.Fatalf("html_inline %s: html hex mismatch", v.Name)
		}
		if r.ExclusionStart != v.ExclusionStart || r.ExclusionLength != v.ExclusionLength {
			t.Fatalf("html_inline %s: exclusion start=%d len=%d", v.Name, r.ExclusionStart, r.ExclusionLength)
		}
		x, _ := ExtractHTML(r.HTML)
		if x == nil || hex.EncodeToString(x.Manifest) != v.ManifestHex {
			t.Fatalf("html_inline %s: extract mismatch", v.Name)
		}
	}

	for _, v := range g.HTMLReference {
		html, err := EmbedHTMLReference(v.HTML, v.URL, v.Newline)
		if err != nil {
			t.Fatalf("html_reference %s: %v", v.Name, err)
		}
		if hex.EncodeToString([]byte(html)) != v.ExpectedHTMLHex {
			t.Fatalf("html_reference %s: html hex mismatch", v.Name)
		}
		x, _ := ExtractHTML(html)
		if x == nil || x.Reference != v.URL {
			t.Fatalf("html_reference %s: extract mismatch", v.Name)
		}
	}
}
