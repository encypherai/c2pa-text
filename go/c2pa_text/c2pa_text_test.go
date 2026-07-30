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
		if issue.Code == ValidationCodeCorruptedWrapper {
			found = true
		}
	}
	if !found {
		t.Fatalf("expected CorruptedWrapper code, got %v", result.Issues)
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
		if issue.Code == ValidationCodeCorruptedWrapper {
			found = true
		}
	}
	if !found {
		t.Fatalf("expected CorruptedWrapper code, got %v", result.Issues)
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

// TestOnlyRegisteredCodesEmitted verifies every status code produced by the
// validators is a member of the C2PA registered validation-status enumeration.
func TestOnlyRegisteredCodesEmitted(t *testing.T) {
	registered := map[ValidationCode]bool{
		ValidationCodeCorruptedWrapper: true,
		ValidationCodeMultipleWrappers: true,
	}
	assertRegistered := func(result *ValidationResult) {
		for _, issue := range result.Issues {
			if !registered[issue.Code] {
				t.Fatalf("emitted unregistered status code %q", issue.Code)
			}
		}
	}

	jumbf := minimalJumbfBox()

	// Direct manifest / JUMBF structural failures.
	empty := ValidateManifest([]byte{}, true, false)
	truncated := ValidateManifest(append([]byte{0, 0, 0, 100}, []byte("jumb")...), true, false)
	badType := ValidateManifest(append([]byte{0, 0, 0, 8}, []byte("xxxx")...), true, false)
	badSize := ValidateManifest(append([]byte{0, 0, 0, 5}, []byte("jumb")...), true, false)
	missingDesc := ValidateJumbfStructure(append([]byte{0, 0, 0, 8}, []byte("jumb")...), true)

	// Wrapper-level failures.
	tooShort := ValidateWrapperBytes([]byte("short"))
	var badVerRaw []byte
	badVerRaw = append(badVerRaw, Magic...)
	badVerRaw = append(badVerRaw, 99)
	verLen := make([]byte, 4)
	binary.BigEndian.PutUint32(verLen, uint32(len(jumbf)))
	badVerRaw = append(badVerRaw, verLen...)
	badVerRaw = append(badVerRaw, jumbf...)
	badVersionWrapper := ValidateWrapperBytes(badVerRaw)

	// Text-level failures.
	signed := EmbedManifest("hello", jumbf)
	multi := ValidateText(signed + EncodeWrapper(jumbf))

	cases := []*ValidationResult{empty, truncated, badType, badSize, missingDesc, tooShort, badVersionWrapper, multi}
	sawFailure := false
	for _, result := range cases {
		if result.Valid {
			t.Fatalf("expected failure, got valid result")
		}
		if len(result.Issues) > 0 {
			sawFailure = true
		}
		assertRegistered(result)
	}
	if !sawFailure {
		t.Fatalf("battery should have produced failures")
	}

	// The multiple-wrappers case must use the registered multipleWrappers code.
	foundMulti := false
	for _, issue := range multi.Issues {
		if issue.Code == ValidationCodeMultipleWrappers {
			foundMulti = true
		}
	}
	if !foundMulti {
		t.Fatalf("expected MultipleWrappers code, got %v", multi.Issues)
	}
}

// The specification fixes the decomposition and the padding byte values so that
// compliant generators emit byte-identical wrappers for the same manifest.
func TestComputePaddingUsesSpecifiedDecomposition(t *testing.T) {
	cases := []struct {
		gap  int
		want []byte
	}{
		{0, nil},
		// b = gap%3, a = (gap-4b)/3, then a bytes of 0x00 and b of 0x10.
		{6, []byte{0x00, 0x00}},
		{7, []byte{0x00, 0x10}},
		{8, []byte{0x10, 0x10}},
		{9, []byte{0x00, 0x00, 0x00}},
		// gap = 12 admits more than one decomposition (four 3-byte selectors or
		// three 4-byte ones); the specified one is four 0x00.
		{12, []byte{0x00, 0x00, 0x00, 0x00}},
	}
	for _, c := range cases {
		got, err := computePadding(c.gap)
		if err != nil {
			t.Fatalf("computePadding(%d) returned error: %v", c.gap, err)
		}
		if len(got) != len(c.want) {
			t.Fatalf("computePadding(%d) = %v, want %v", c.gap, got, c.want)
		}
		for i := range c.want {
			if got[i] != c.want[i] {
				t.Fatalf("computePadding(%d) = %v, want %v", c.gap, got, c.want)
			}
		}
	}
}

func TestComputePaddingEncodesToExactByteCount(t *testing.T) {
	for gap := 6; gap <= 200; gap++ {
		padding, err := computePadding(gap)
		if err != nil {
			t.Fatalf("computePadding(%d) returned error: %v", gap, err)
		}
		encoded := ""
		for _, b := range padding {
			r, err := byteToVS(b)
			if err != nil {
				t.Fatalf("byteToVS(%#x) returned error: %v", b, err)
			}
			encoded += string(r)
		}
		if len(encoded) != gap {
			t.Fatalf("gap %d encoded to %d bytes", gap, len(encoded))
		}
	}
}

func TestComputePaddingRejectsUnrepresentableGaps(t *testing.T) {
	// 1, 2 and 5 are not expressible as 3a + 4b.
	for _, gap := range []int{1, 2, 5} {
		if _, err := computePadding(gap); err == nil {
			t.Fatalf("computePadding(%d) should have failed", gap)
		}
	}
}
