// C2PA HTML embedding (C2PA Technical Specification 2.4, Appendix A.7).
//
// Associates a C2PA Manifest Store with an HTML document using one of the two
// methods the spec defines, both keyed on the IANA media type "application/c2pa":
//
//   - Inline: a <script type="application/c2pa"> element in the <head> whose
//     content is the Base64-encoded Manifest Store.
//   - Referenced (preferred): a <link rel="c2pa-manifest" href="..."> element in
//     the <head> pointing at an external Manifest Store.
//
// A document shall carry at most one association; more than one is the
// manifest.html.multipleManifests validation failure (spec A.7.1).
//
// Separate pipeline from the unstructured (A.8) and structured (A.9) methods.
// Wire-compatible (byte-identical output) with the Rust, Python and TypeScript
// modules for the same inputs.
package c2pa_text

import (
	"encoding/base64"
	"strings"
)

const (
	// C2PAMediaType is the IANA media type for a C2PA Manifest Store.
	C2PAMediaType   = "application/c2pa"
	htmlScriptOpen  = `<script type="application/c2pa">`
	htmlScriptClose = "</script>"
	htmlHeadClose   = "</head>"

	// HTMLMultipleManifests is the C2PA validation status code for a document
	// carrying more than one manifest association (spec A.7.1).
	HTMLMultipleManifests = "manifest.html.multipleManifests"
	// HTMLNoHead is the embed-time error code when the host document has no
	// </head> in which to place the manifest element.
	HTMLNoHead = "html.noHead"
)

// HTMLError is an HTML embedding/extraction error carrying a status code.
type HTMLError struct {
	Code    string
	Message string
}

func (e *HTMLError) Error() string { return e.Code }

// HTML embedding/extraction errors.
var (
	ErrHTMLMultipleManifests = &HTMLError{
		Code:    HTMLMultipleManifests,
		Message: "More than one C2PA manifest association in HTML document",
	}
	ErrHTMLNoHead = &HTMLError{
		Code:    HTMLNoHead,
		Message: "No </head> found to place the C2PA manifest element",
	}
)

// HTMLEmbed is the result of embedding an inline manifest into HTML.
type HTMLEmbed struct {
	HTML            string
	ExclusionStart  int
	ExclusionLength int
}

// HTMLExtraction is the result of extracting a manifest association from HTML.
type HTMLExtraction struct {
	// Method is "inline" (script element) or "reference" (link element).
	Method string
	// Manifest holds decoded Manifest Store bytes for the inline method.
	Manifest []byte
	// Reference holds the external manifest URL for the reference method.
	Reference string
}

// BuildHTMLScript builds a <script type="application/c2pa">...</script> element
// whose content is the Base64-encoded Manifest Store (spec A.7.1.1).
func BuildHTMLScript(manifestBytes []byte) string {
	return htmlScriptOpen + base64.StdEncoding.EncodeToString(manifestBytes) + htmlScriptClose
}

// BuildHTMLLink builds a <link rel="c2pa-manifest" href="..." type="application/c2pa">
// element referencing an external Manifest Store (spec A.7.1.2).
func BuildHTMLLink(url string) string {
	return `<link rel="c2pa-manifest" href="` + url + `" type="application/c2pa">`
}

// EmbedHTMLInline embeds a Manifest Store inline as a <script> element placed
// just before </head> and returns the document plus the c2pa.hash.data exclusion
// range covering the element (spec A.7.1.1, A.7.1.3).
func EmbedHTMLInline(html string, manifestBytes []byte, newline string) (HTMLEmbed, error) {
	element := BuildHTMLScript(manifestBytes)
	idx := strings.Index(html, htmlHeadClose)
	if idx == -1 {
		return HTMLEmbed{}, ErrHTMLNoHead
	}
	out := html[:idx] + element + newline + html[idx:]
	return HTMLEmbed{HTML: out, ExclusionStart: idx, ExclusionLength: len(element)}, nil
}

// EmbedHTMLReference embeds a reference to an external Manifest Store as a <link>
// element placed just before </head> (spec A.7.1.2). The referenced method's
// hard binding has no exclusion range (the hash covers the whole document).
func EmbedHTMLReference(html, url, newline string) (string, error) {
	element := BuildHTMLLink(url)
	idx := strings.Index(html, htmlHeadClose)
	if idx == -1 {
		return "", ErrHTMLNoHead
	}
	return html[:idx] + element + newline + html[idx:], nil
}

func findScriptContents(html string) []string {
	var results []string
	pos := 0
	for {
		i := strings.Index(html[pos:], "<script")
		if i == -1 {
			break
		}
		i += pos
		gt := strings.Index(html[i:], ">")
		if gt == -1 {
			break
		}
		gt += i
		tag := html[i : gt+1]
		if strings.Contains(tag, `type="application/c2pa"`) {
			end := strings.Index(html[gt+1:], htmlScriptClose)
			if end != -1 {
				end += gt + 1
				results = append(results, html[gt+1:end])
				pos = end + len(htmlScriptClose)
				continue
			}
		}
		pos = gt + 1
	}
	return results
}

func findLinkTags(html string) []string {
	var results []string
	pos := 0
	for {
		i := strings.Index(html[pos:], "<link")
		if i == -1 {
			break
		}
		i += pos
		gt := strings.Index(html[i:], ">")
		if gt == -1 {
			break
		}
		gt += i
		tag := html[i : gt+1]
		if strings.Contains(tag, `rel="c2pa-manifest"`) {
			results = append(results, tag)
		}
		pos = gt + 1
	}
	return results
}

func hrefOf(tag string) string {
	const marker = `href="`
	i := strings.Index(tag, marker)
	if i == -1 {
		return ""
	}
	start := i + len(marker)
	end := strings.Index(tag[start:], `"`)
	if end == -1 {
		return ""
	}
	return tag[start : start+end]
}

// ExtractHTML extracts a manifest association from an HTML document (spec
// A.7.1.4). Returns (nil, nil) if no association is present, and
// (nil, ErrHTMLMultipleManifests) if more than one association is found.
func ExtractHTML(html string) (*HTMLExtraction, error) {
	scripts := findScriptContents(html)
	links := findLinkTags(html)
	total := len(scripts) + len(links)
	if total == 0 {
		return nil, nil
	}
	if total > 1 {
		return nil, ErrHTMLMultipleManifests
	}
	if len(scripts) > 0 {
		manifest, err := base64.StdEncoding.DecodeString(strings.TrimSpace(scripts[0]))
		if err != nil {
			manifest = nil
		}
		return &HTMLExtraction{Method: "inline", Manifest: manifest}, nil
	}
	return &HTMLExtraction{Method: "reference", Reference: hrefOf(links[0])}, nil
}
