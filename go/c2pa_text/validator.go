package c2pa_text

import (
	"encoding/binary"
	"fmt"

	"golang.org/x/text/unicode/norm"
)

// JUMBF Constants (ISO/IEC 19566-5)
var (
	JumbfSuperboxType      = []byte("jumb")
	JumbfDescType          = []byte("jumd")
	C2PAManifestStoreUUID  = []byte{
		0x63, 0x32, 0x70, 0x61, 0x00, 0x11, 0x00, 0x10,
		0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
	}
)

// ValidationCode is a registered C2PA validation status code emitted for text
// manifests. Only members of the C2PA registered validation-status enumeration
// are produced, so verifiers and integration partners get machine-readable
// interop. The specific structural reason for a failure is carried in
// ValidationIssue.Message (the C2PA model of a coarse status code plus a
// human-readable explanation).
type ValidationCode string

const (
	// ValidationCodeValid indicates no structural issues were found.
	ValidationCodeValid ValidationCode = "valid"

	// ValidationCodeCorruptedWrapper indicates a C2PATextManifestWrapper was
	// located but is malformed or incomplete: bad magic, unsupported version,
	// truncated/length mismatch, or an invalid embedded JUMBF manifest store.
	ValidationCodeCorruptedWrapper ValidationCode = "manifest.text.corruptedWrapper"

	// ValidationCodeMultipleWrappers indicates more than one valid
	// C2PATextManifestWrapper was found in the text.
	ValidationCodeMultipleWrappers ValidationCode = "manifest.text.multipleWrappers"
)

// ValidationIssue represents a single validation issue.
type ValidationIssue struct {
	Code    ValidationCode
	Message string
	Offset  int
	Context string
}

func (i ValidationIssue) String() string {
	return fmt.Sprintf("[%s] %s", i.Code, i.Message)
}

// ValidationResult contains the result of manifest validation.
type ValidationResult struct {
	Valid          bool
	Issues         []ValidationIssue
	ManifestBytes  []byte
	JumbfBytes     []byte
	Version        int
	DeclaredLength uint32
	ActualLength   int
}

// NewValidationResult creates a new valid result.
func NewValidationResult() *ValidationResult {
	return &ValidationResult{
		Valid:  true,
		Issues: make([]ValidationIssue, 0),
	}
}

// AddIssue adds a validation issue and marks the result as invalid.
func (r *ValidationResult) AddIssue(code ValidationCode, message string, offset int, context string) {
	r.Issues = append(r.Issues, ValidationIssue{
		Code:    code,
		Message: message,
		Offset:  offset,
		Context: context,
	})
	r.Valid = false
}

// PrimaryCode returns the most severe validation code.
func (r *ValidationResult) PrimaryCode() ValidationCode {
	if len(r.Issues) == 0 {
		return ValidationCodeValid
	}
	return r.Issues[0].Code
}

func (r *ValidationResult) String() string {
	if r.Valid {
		return "Validation passed: manifest is structurally compliant"
	}
	result := "Validation failed:\n"
	for _, issue := range r.Issues {
		result += fmt.Sprintf("  - %s\n", issue.String())
	}
	return result
}

func bytesEqual(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

// ValidateJumbfStructure validates basic JUMBF box structure.
func ValidateJumbfStructure(jumbfBytes []byte, strict bool) *ValidationResult {
	result := NewValidationResult()
	result.JumbfBytes = jumbfBytes

	if len(jumbfBytes) == 0 {
		result.AddIssue(ValidationCodeCorruptedWrapper, "JUMBF content is empty", 0, "")
		return result
	}

	// Minimum JUMBF box: 8 bytes header (size + type)
	if len(jumbfBytes) < 8 {
		result.AddIssue(
			ValidationCodeCorruptedWrapper,
			fmt.Sprintf("JUMBF too short for box header: %d bytes, minimum 8", len(jumbfBytes)),
			0, "",
		)
		return result
	}

	// Parse first box header (Big Endian)
	boxSize := binary.BigEndian.Uint32(jumbfBytes[0:4])
	boxType := jumbfBytes[4:8]

	var effectiveSize int
	var headerSize int

	if boxSize == 0 {
		// Size 0 means "extends to end of file"
		effectiveSize = len(jumbfBytes)
		headerSize = 8
	} else if boxSize == 1 {
		// Extended size (64-bit)
		if len(jumbfBytes) < 16 {
			result.AddIssue(
				ValidationCodeCorruptedWrapper,
				"Extended box size declared but not enough bytes for 64-bit size field",
				0, "",
			)
			return result
		}
		effectiveSize = int(binary.BigEndian.Uint64(jumbfBytes[8:16]))
		headerSize = 16
	} else if boxSize < 8 {
		result.AddIssue(
			ValidationCodeCorruptedWrapper,
			fmt.Sprintf("Invalid box size: %d (minimum is 8)", boxSize),
			0, "",
		)
		return result
	} else {
		effectiveSize = int(boxSize)
		headerSize = 8
	}

	// Check if we have enough bytes
	if len(jumbfBytes) < effectiveSize {
		result.AddIssue(
			ValidationCodeCorruptedWrapper,
			fmt.Sprintf("JUMBF truncated: declared size %d, actual %d", effectiveSize, len(jumbfBytes)),
			0, "",
		)
		return result
	}

	// Check for JUMBF superbox type
	if !bytesEqual(boxType, JumbfSuperboxType) {
		result.AddIssue(
			ValidationCodeCorruptedWrapper,
			fmt.Sprintf("Expected JUMBF superbox type 'jumb', got '%s'", string(boxType)),
			4,
			fmt.Sprintf("box_type=%x", boxType),
		)
		return result
	}

	if strict {
		// Check for description box (jumd)
		if len(jumbfBytes) < headerSize+8 {
			result.AddIssue(
				ValidationCodeCorruptedWrapper,
				"JUMBF superbox too short to contain description box",
				headerSize, "",
			)
			return result
		}

		descType := jumbfBytes[headerSize+4 : headerSize+8]
		if !bytesEqual(descType, JumbfDescType) {
			result.AddIssue(
				ValidationCodeCorruptedWrapper,
				fmt.Sprintf("Expected description box 'jumd', got '%s'", string(descType)),
				headerSize+4, "",
			)
			return result
		}

		// Check for C2PA UUID
		uuidOffset := headerSize + 8
		if len(jumbfBytes) >= uuidOffset+16 {
			foundUuid := jumbfBytes[uuidOffset : uuidOffset+16]
			if !bytesEqual(foundUuid, C2PAManifestStoreUUID) {
				result.AddIssue(
					ValidationCodeCorruptedWrapper,
					"Invalid C2PA manifest store UUID",
					uuidOffset,
					fmt.Sprintf("expected=%x, found=%x", C2PAManifestStoreUUID, foundUuid),
				)
			}
		}
	}

	return result
}

// ValidateManifest validates a C2PA manifest before embedding.
func ValidateManifest(manifestBytes []byte, validateJumbf bool, strict bool) *ValidationResult {
	result := NewValidationResult()
	result.ManifestBytes = manifestBytes

	if len(manifestBytes) == 0 {
		result.AddIssue(ValidationCodeCorruptedWrapper, "Manifest bytes are empty", -1, "")
		return result
	}

	result.ActualLength = len(manifestBytes)

	if validateJumbf {
		jumbfResult := ValidateJumbfStructure(manifestBytes, strict)
		if !jumbfResult.Valid {
			result.Issues = append(result.Issues, jumbfResult.Issues...)
			result.Valid = false
		}
	}

	return result
}

// ValidateWrapperBytes validates a pre-encoded C2PATextManifestWrapper.
func ValidateWrapperBytes(wrapperBytes []byte) *ValidationResult {
	result := NewValidationResult()

	if len(wrapperBytes) < HeaderSize {
		result.AddIssue(
			ValidationCodeCorruptedWrapper,
			fmt.Sprintf("Wrapper too short: %d bytes, minimum %d", len(wrapperBytes), HeaderSize),
			0, "",
		)
		return result
	}

	// Check magic
	magic := wrapperBytes[0:8]
	if !bytesEqual(magic, Magic) {
		result.AddIssue(
			ValidationCodeCorruptedWrapper,
			fmt.Sprintf("Invalid magic: expected 'C2PATXT\\0', got '%s'", string(magic)),
			0, "",
		)
		return result
	}

	// Check version
	version := int(wrapperBytes[8])
	result.Version = version
	if version != Version {
		result.AddIssue(
			ValidationCodeCorruptedWrapper,
			fmt.Sprintf("Unsupported version: %d, expected %d", version, Version),
			8, "",
		)
		return result
	}

	// Check length
	declaredLength := binary.BigEndian.Uint32(wrapperBytes[9:13])
	result.DeclaredLength = declaredLength

	actualJumbfLength := len(wrapperBytes) - HeaderSize
	result.ActualLength = actualJumbfLength

	// Actual bytes after header must be >= declared. Trailing bytes beyond
	// manifestLength are padding (spec says decoders use manifestLength to
	// extract the manifest and ignore trailing padding).
	if int(declaredLength) > actualJumbfLength {
		result.AddIssue(
			ValidationCodeCorruptedWrapper,
			fmt.Sprintf("Length mismatch: declares %d bytes, only %d available (truncated)", declaredLength, actualJumbfLength),
			9, "",
		)
		return result
	}

	// Extract the declared manifest bytes (ignore trailing padding)
	jumbfBytes := wrapperBytes[HeaderSize : HeaderSize+int(declaredLength)]
	result.JumbfBytes = jumbfBytes
	result.ManifestBytes = jumbfBytes

	jumbfResult := ValidateJumbfStructure(jumbfBytes, false)
	if !jumbfResult.Valid {
		result.Issues = append(result.Issues, jumbfResult.Issues...)
		result.Valid = false
	}

	return result
}

// ValidateText validates a text asset for C2PA text wrapper compliance.
//
// Scans the full text for C2PA wrappers and reports structural issues:
//   - Multiple wrappers (spec requires zero or one)
//   - Corrupted, truncated, or malformed wrappers
//   - Invalid magic bytes or unsupported version
//   - JUMBF structural issues in the embedded manifest
//
// This is the recommended validation entry point for text assets.
func ValidateText(text string) *ValidationResult {
	result := NewValidationResult()

	normalized := norm.NFC.String(text)
	runes := []rune(normalized)

	type wrapperMatch struct {
		charIndex int
		raw       []byte
	}
	var validWrappers []wrapperMatch

	i := 0
	for i < len(runes) {
		if runes[i] == ZWNBSP {
			var rawBytes []byte
			j := i + 1

			for j < len(runes) {
				b, ok := vsToByte(runes[j])
				if !ok {
					break
				}
				rawBytes = append(rawBytes, b)
				j++
			}

			// Check for valid C2PA header (magic bytes match)
			if len(rawBytes) >= HeaderSize {
				magicMatch := true
				for k := 0; k < 8; k++ {
					if rawBytes[k] != Magic[k] {
						magicMatch = false
						break
					}
				}
				if magicMatch {
					validWrappers = append(validWrappers, wrapperMatch{
						charIndex: i,
						raw:       rawBytes,
					})
				}
			}

			i = j
			continue
		}
		i++
	}

	if len(validWrappers) == 0 {
		// No wrapper found is valid (wrapper is optional per spec).
		return result
	}

	if len(validWrappers) > 1 {
		// Convert rune index of second wrapper to byte offset
		byteOffset := len(string(runes[:validWrappers[1].charIndex]))
		result.AddIssue(
			ValidationCodeMultipleWrappers,
			fmt.Sprintf("Found %d valid C2PA text wrappers (spec requires at most one)", len(validWrappers)),
			byteOffset, "",
		)
	}

	// Validate each wrapper structurally
	for _, w := range validWrappers {
		wrapperResult := ValidateWrapperBytes(w.raw)
		if !wrapperResult.Valid {
			result.Issues = append(result.Issues, wrapperResult.Issues...)
			result.Valid = false
		}
	}

	return result
}
