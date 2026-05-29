package c2pa_text

import (
	"encoding/binary"
	"testing"

	"golang.org/x/text/unicode/norm"
)

func TestExtractManifestOffsetsAreNFCUtf8ByteOffsets(t *testing.T) {
	manifest := make([]byte, 8)
	binary.BigEndian.PutUint32(manifest[0:4], 8)
	copy(manifest[4:8], []byte("jumb"))

	decomposed := "e\u0301"
	embedded := EmbedManifest(decomposed, manifest)

	extracted, clean, offset, length, err := ExtractManifest(embedded)
	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}
	if extracted == nil {
		t.Fatalf("expected extracted bytes")
	}
	if string(extracted) != string(manifest) {
		t.Fatalf("extracted manifest mismatch")
	}

	normalized := norm.NFC.String(decomposed)
	expectedOffset := len([]byte(normalized))
	expectedLength := len([]byte(embedded)) - expectedOffset

	if clean != normalized {
		t.Fatalf("clean text mismatch: got %q expected %q", clean, normalized)
	}
	if offset != expectedOffset {
		t.Fatalf("offset mismatch: got %d expected %d", offset, expectedOffset)
	}
	if length != expectedLength {
		t.Fatalf("length mismatch: got %d expected %d", length, expectedLength)
	}
}

func TestExtractManifestMultipleWrappersErrors(t *testing.T) {
	manifest := make([]byte, 8)
	binary.BigEndian.PutUint32(manifest[0:4], 8)
	copy(manifest[4:8], []byte("jumb"))

	base := EmbedManifest("hello", manifest)
	double := base + EncodeWrapper(manifest)

	_, _, _, _, err := ExtractManifest(double)
	if err == nil {
		t.Fatalf("expected error")
	}
	if err != ErrMultipleWrappers {
		t.Fatalf("expected ErrMultipleWrappers, got %v", err)
	}
}

// --- ValidateText tests ---

func minimalJumbfBox() []byte {
	// Minimal valid JUMBF superbox: size=8, type="jumb"
	manifest := make([]byte, 8)
	binary.BigEndian.PutUint32(manifest[0:4], 8)
	copy(manifest[4:8], []byte("jumb"))
	return manifest
}

func TestValidateTextPlainNoWrapper(t *testing.T) {
	result := ValidateText("Hello, World!")
	if !result.Valid {
		t.Fatalf("expected valid, got issues: %v", result.Issues)
	}
	if len(result.Issues) != 0 {
		t.Fatalf("expected 0 issues, got %d", len(result.Issues))
	}
}

func TestValidateTextSingleValidWrapper(t *testing.T) {
	manifest := minimalJumbfBox()
	embedded := EmbedManifest("Hello, World!", manifest)
	result := ValidateText(embedded)
	if !result.Valid {
		t.Fatalf("expected valid, got issues: %v", result.Issues)
	}
}

func TestValidateTextMultipleWrappers(t *testing.T) {
	manifest := minimalJumbfBox()
	wrapper := EncodeWrapper(manifest)
	text := "Hello" + wrapper + " more text " + wrapper
	result := ValidateText(text)
	if result.Valid {
		t.Fatalf("expected invalid for multiple wrappers")
	}
	found := false
	for _, issue := range result.Issues {
		if issue.Code == ValidationCodeMultipleWrappers {
			found = true
		}
	}
	if !found {
		t.Fatalf("expected MultipleWrappers code, got %v", result.Issues)
	}
}

func TestValidateTextBadVersion(t *testing.T) {
	// Build wrapper with version=99 instead of 1
	var raw []byte
	raw = append(raw, Magic...)
	raw = append(raw, 99) // bad version
	// Length = 8
	lenBytes := make([]byte, 4)
	binary.BigEndian.PutUint32(lenBytes, 8)
	raw = append(raw, lenBytes...)
	// 8 body bytes (valid JUMBF box)
	raw = append(raw, minimalJumbfBox()...)

	// Encode as VS string
	wrapper := string([]rune{ZWNBSP})
	for _, b := range raw {
		r, _ := byteToVS(b)
		wrapper += string(r)
	}

	text := "Some text" + wrapper
	result := ValidateText(text)
	if result.Valid {
		t.Fatalf("expected invalid for bad version")
	}
	found := false
	for _, issue := range result.Issues {
		if issue.Code == ValidationCodeUnsupportedVersion {
			found = true
		}
	}
	if !found {
		t.Fatalf("expected UnsupportedVersion code, got %v", result.Issues)
	}
}

func TestValidateTextLengthMismatch(t *testing.T) {
	// Build wrapper declaring 100 bytes but providing only 8
	var raw []byte
	raw = append(raw, Magic...)
	raw = append(raw, byte(Version))
	lenBytes := make([]byte, 4)
	binary.BigEndian.PutUint32(lenBytes, 100) // declares 100
	raw = append(raw, lenBytes...)
	// Only 8 body bytes
	raw = append(raw, minimalJumbfBox()...)

	wrapper := string([]rune{ZWNBSP})
	for _, b := range raw {
		r, _ := byteToVS(b)
		wrapper += string(r)
	}

	text := "Some text" + wrapper
	result := ValidateText(text)
	if result.Valid {
		t.Fatalf("expected invalid for length mismatch")
	}
	found := false
	for _, issue := range result.Issues {
		if issue.Code == ValidationCodeLengthMismatch {
			found = true
		}
	}
	if !found {
		t.Fatalf("expected LengthMismatch code, got %v", result.Issues)
	}
}

func TestValidateTextTrailingPaddingAccepted(t *testing.T) {
	// Build wrapper declaring 8 bytes but providing 12 (4 padding)
	var raw []byte
	raw = append(raw, Magic...)
	raw = append(raw, byte(Version))
	lenBytes := make([]byte, 4)
	binary.BigEndian.PutUint32(lenBytes, 8) // declares 8
	raw = append(raw, lenBytes...)
	// 8 manifest bytes (valid JUMBF)
	raw = append(raw, minimalJumbfBox()...)
	// 4 trailing padding bytes
	raw = append(raw, 0, 0, 0, 0)

	wrapper := string([]rune{ZWNBSP})
	for _, b := range raw {
		r, _ := byteToVS(b)
		wrapper += string(r)
	}

	text := "Some text" + wrapper
	result := ValidateText(text)
	if !result.Valid {
		t.Fatalf("expected valid (trailing padding should be accepted), got issues: %v", result.Issues)
	}
}

func TestValidateTextNFCNormalization(t *testing.T) {
	manifest := minimalJumbfBox()
	// "cafe" with combining accent (NFD form)
	nfdText := "caf\u0065\u0301"
	embedded := EmbedManifest(nfdText, manifest)
	result := ValidateText(embedded)
	if !result.Valid {
		t.Fatalf("expected valid after NFC normalization, got issues: %v", result.Issues)
	}
}

func TestValidateTextBadMagicIgnored(t *testing.T) {
	// ZWNBSP followed by 13 VS chars encoding zeros (not C2PA magic)
	wrapper := string([]rune{ZWNBSP})
	r, _ := byteToVS(0)
	for i := 0; i < 13; i++ {
		wrapper += string(r)
	}
	text := "Hello" + wrapper
	result := ValidateText(text)
	if !result.Valid {
		t.Fatalf("expected valid (bad magic should be ignored), got issues: %v", result.Issues)
	}
}
