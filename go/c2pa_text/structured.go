// C2PA Structured Text embedding (C2PA Technical Specification 2.4, Appendix A.9).
//
// Associates a C2PA Manifest Store with a *structured* text asset -- source
// code, configuration files (YAML, TOML, INI), markup (Markdown, AsciiDoc,
// LaTeX), XML, and similar formats that support a comment or front-matter
// convention -- using an ASCII Armour-style block (modelled on OpenPGP ASCII
// Armor, RFC 4880 sec 6.2) delimited by:
//
//	-----BEGIN C2PA MANIFEST----- <manifest-reference> -----END C2PA MANIFEST-----
//
// The <manifest-reference> is either a URL to an external C2PA Manifest Store
// (preferred) or a "data:application/c2pa;base64,..." URI embedding the store.
//
// This is a separate pipeline from the unstructured (Unicode Variation Selector)
// method in c2pa_text.go (Appendix A.8). Neither pipeline is restricted to a
// fixed set of media types: the implementer chooses which method to use for a
// given asset. RecommendedMethod offers an advisory mapping but is informative.
//
// Wire-compatible (byte-identical output) with the Rust, Python and TypeScript
// c2pa-text structured modules for the same inputs.
package c2pa_text

import (
	"encoding/base64"
	"strings"
)

// Fixed ASCII Armour-style delimiters (spec A.9.3).
const (
	BeginDelimiter = "-----BEGIN C2PA MANIFEST-----"
	EndDelimiter   = "-----END C2PA MANIFEST-----"
	// DataURIPrefix is the prefix of a data: URI carrying a Base64-encoded
	// C2PA Manifest Store.
	DataURIPrefix = "data:application/c2pa;base64,"
)

// Placement selects where to place the manifest block relative to the host
// text (spec A.9.3.1).
type Placement int

const (
	// PlacementStart places the block at the beginning of the file (preferred).
	PlacementStart Placement = iota
	// PlacementEnd places the block at the end of the file, used when the first
	// line is reserved by the host format (e.g. a "#!" shebang or "<?xml ?>").
	PlacementEnd
)

// Method is an advisory recommendation of which embedding method best fits a
// media type, per the C2PA 2.4 spec text families.
type Method int

const (
	// MethodNone indicates no defined text embedding method for the media type.
	MethodNone Method = iota
	// MethodUnstructured is the Unicode Variation Selector wrapper (A.8).
	MethodUnstructured
	// MethodStructured is the ASCII Armour comment/front-matter block (A.9).
	MethodStructured
	// MethodHTML is the HTML script/link method (A.7), not implemented here.
	MethodHTML
	// MethodSVG is the SVG metadata method (A.3.3), not implemented here.
	MethodSVG
)

// StructuredEmbed is the result of embedding a structured-text manifest block.
type StructuredEmbed struct {
	// Text is the host text with the manifest block inserted.
	Text string
	// ExclusionStart is the byte offset of the c2pa.hash.data exclusion range.
	ExclusionStart int
	// ExclusionLength is the byte length of the c2pa.hash.data exclusion range.
	ExclusionLength int
}

// StructuredExtraction is the result of extracting a structured-text manifest
// block.
type StructuredExtraction struct {
	// Reference is the manifest reference between the delimiters (URL or data:
	// URI), trimmed of surrounding whitespace.
	Reference string
	// Manifest holds the decoded Manifest Store bytes, non-nil only when
	// Reference is a "data:application/c2pa;base64,..." URI.
	Manifest []byte
}

// StructuredError is an extraction failure carrying the normative C2PA
// validation status code (spec A.9.5).
type StructuredError struct {
	Code    string
	Message string
}

func (e *StructuredError) Error() string { return e.Code }

// Structured extraction errors, each mapping to a C2PA failure status code.
var (
	ErrNoManifest = &StructuredError{
		Code:    "manifest.structuredText.noManifest",
		Message: "No C2PA manifest block delimiters found",
	}
	ErrMultipleReferences = &StructuredError{
		Code:    "manifest.structuredText.multipleReferences",
		Message: "Multiple C2PA manifest blocks found",
	}
	ErrEmptyReference = &StructuredError{
		Code:    "manifest.structuredText.emptyReference",
		Message: "Manifest reference between delimiters is empty",
	}
)

// EncodeDataURI builds a "data:application/c2pa;base64,..." URI for a Manifest
// Store (spec A.9.3.1), using standard Base64 (RFC 4648 sec 4, padded).
func EncodeDataURI(manifestBytes []byte) string {
	return DataURIPrefix + base64.StdEncoding.EncodeToString(manifestBytes)
}

// DecodeDataURI decodes a "data:application/c2pa;base64,..." reference into
// Manifest Store bytes. The bool result is false if reference is not such a
// data: URI or the Base64 payload is invalid.
func DecodeDataURI(reference string) ([]byte, bool) {
	if !strings.HasPrefix(reference, DataURIPrefix) {
		return nil, false
	}
	payload := strings.TrimSpace(reference[len(DataURIPrefix):])
	decoded, err := base64.StdEncoding.DecodeString(payload)
	if err != nil {
		return nil, false
	}
	return decoded, true
}

// BuildManifestBlock builds a single-line manifest block (spec A.9.3.1):
//
//	<commentPrefix> -----BEGIN C2PA MANIFEST----- <reference> -----END C2PA MANIFEST----- <commentSuffix>
//
// commentSuffix is appended (space-separated) only when non-empty.
func BuildManifestBlock(reference, commentPrefix, commentSuffix string) string {
	block := commentPrefix + " " + BeginDelimiter + " " + reference + " " + EndDelimiter
	if commentSuffix != "" {
		block += " " + commentSuffix
	}
	return block
}

// BuildManifestBlockMultiline builds a multi-line manifest block for placement
// inside host front matter (spec A.9.3.2). The host front-matter fences (e.g.
// "---") are not part of the C2PA block and must be supplied by the caller.
func BuildManifestBlockMultiline(reference, newline string) string {
	return BeginDelimiter + newline + reference + newline + EndDelimiter
}

// EmbedStructured embeds a manifest block into structured text using the
// single-line comment form (spec A.9.3.1) and returns the resulting text and
// the c2pa.hash.data exclusion range to bind it (spec A.9.4).
//
// newline is the host line terminator -- "\n" (LF) or "\r\n" (CRLF).
func EmbedStructured(text, reference, commentPrefix, commentSuffix string, placement Placement, newline string) StructuredEmbed {
	block := BuildManifestBlock(reference, commentPrefix, commentSuffix)
	if placement == PlacementStart {
		return StructuredEmbed{
			Text:            block + newline + text,
			ExclusionStart:  0,
			ExclusionLength: len(block) + len(newline),
		}
	}
	start := len(text)
	return StructuredEmbed{
		Text:            text + newline + block,
		ExclusionStart:  start,
		ExclusionLength: len(newline) + len(block),
	}
}

// ExtractStructured extracts a manifest reference from structured text (spec
// A.9.5). Form-agnostic: the reference is whatever appears between the single
// pair of delimiters, trimmed, so both single-line and front-matter forms are
// handled. Returns a *StructuredError on noManifest / multipleReferences /
// emptyReference.
func ExtractStructured(text string) (StructuredExtraction, error) {
	beginCount := strings.Count(text, BeginDelimiter)
	endCount := strings.Count(text, EndDelimiter)
	if beginCount == 0 || endCount == 0 {
		return StructuredExtraction{}, ErrNoManifest
	}
	if beginCount > 1 || endCount > 1 {
		return StructuredExtraction{}, ErrMultipleReferences
	}
	begin := strings.Index(text, BeginDelimiter) + len(BeginDelimiter)
	end := strings.Index(text, EndDelimiter)
	if end <= begin {
		return StructuredExtraction{}, ErrNoManifest
	}
	reference := strings.TrimSpace(text[begin:end])
	if reference == "" {
		return StructuredExtraction{}, ErrEmptyReference
	}
	manifest, _ := DecodeDataURI(reference)
	return StructuredExtraction{Reference: reference, Manifest: manifest}, nil
}

// RecommendedMethod returns an advisory embedding method for a media type, per
// the C2PA 2.4 spec text families. Returns MethodNone for media types with no
// defined text embedding method. Informative only.
func RecommendedMethod(mime string) Method {
	switch mime {
	case "text/plain", "text/markdown", "text/csv", "application/json":
		// Unstructured family (A.8). JSON and CSV have no comment/front-matter
		// syntax, so the structured method (A.9) cannot apply to them.
		return MethodUnstructured
	case "text/xml", "application/xml", "application/xhtml+xml":
		// Structured family (A.9), via XML comment syntax `<!-- -->`.
		return MethodStructured
	case "text/html":
		return MethodHTML
	case "image/svg+xml":
		return MethodSVG
	}
	if strings.HasPrefix(mime, "text/") {
		return MethodStructured
	}
	return MethodNone
}
