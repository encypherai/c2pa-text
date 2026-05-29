/**
 * C2PA HTML embedding (C2PA Technical Specification 2.4, Appendix A.7).
 *
 * Associates a C2PA Manifest Store with an HTML document using one of the two
 * methods the spec defines, both keyed on the IANA media type `application/c2pa`:
 *
 * - **Inline** -- a `<script type="application/c2pa">` element in the `<head>`
 *   whose content is the Base64-encoded Manifest Store.
 * - **Referenced** (preferred) -- a `<link rel="c2pa-manifest" href="...">`
 *   element in the `<head>` pointing at an external Manifest Store.
 *
 * A document shall carry at most one association; more than one is the
 * `manifest.html.multipleManifests` validation failure (spec A.7.1).
 *
 * Separate pipeline from the unstructured (A.8) and structured (A.9) methods.
 * Wire-compatible (byte-identical output) with the Rust, Python and Go modules.
 */

import { base64Decode, base64Encode, utf8Length } from './b64';

export const C2PA_MEDIA_TYPE = 'application/c2pa';
const SCRIPT_OPEN = '<script type="application/c2pa">';
const SCRIPT_CLOSE = '</script>';
const HEAD_CLOSE = '</head>';

/** Validation status code (spec A.7.1). */
export const MULTIPLE_MANIFESTS = 'manifest.html.multipleManifests';
/** Embed-time error code: the host document has no `</head>`. */
export const NO_HEAD = 'html.noHead';

/** HTML embedding/extraction error; `code` is the C2PA status or embed-time code. */
export class HtmlError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.code = code;
    this.name = 'HtmlError';
  }
}

/** Result of embedding an inline manifest into HTML. */
export interface HtmlEmbed {
  html: string;
  /** Byte offset of the `c2pa.hash.data` exclusion range (spec A.7.1.3). */
  exclusionStart: number;
  /** Byte length of the exclusion range (the entire `<script>` element). */
  exclusionLength: number;
}

/** Result of extracting a manifest association from HTML. */
export interface HtmlExtraction {
  method: 'inline' | 'reference';
  manifest: Uint8Array | null;
  reference: string | null;
}

/** Build a `<script type="application/c2pa">...</script>` element (A.7.1.1). */
export function buildHtmlScript(manifestBytes: Uint8Array): string {
  return SCRIPT_OPEN + base64Encode(manifestBytes) + SCRIPT_CLOSE;
}

/** Build a `<link rel="c2pa-manifest" href="..." type="application/c2pa">` (A.7.1.2). */
export function buildHtmlLink(url: string): string {
  return `<link rel="c2pa-manifest" href="${url}" type="application/c2pa">`;
}

function headCloseIndex(html: string): number {
  const idx = html.indexOf(HEAD_CLOSE);
  if (idx === -1) {
    throw new HtmlError(NO_HEAD, 'No </head> found to place the C2PA manifest element');
  }
  return idx;
}

/**
 * Embed a Manifest Store inline as a `<script>` element placed just before
 * `</head>`, returning the document and the `c2pa.hash.data` exclusion range
 * covering the element (spec A.7.1.1, A.7.1.3).
 */
export function embedHtmlInline(
  html: string,
  manifestBytes: Uint8Array,
  newline = '\n',
): HtmlEmbed {
  const element = buildHtmlScript(manifestBytes);
  const idx = headCloseIndex(html);
  const out = html.slice(0, idx) + element + newline + html.slice(idx);
  return {
    html: out,
    exclusionStart: utf8Length(html.slice(0, idx)),
    exclusionLength: utf8Length(element),
  };
}

/**
 * Embed a reference to an external Manifest Store as a `<link>` element placed
 * just before `</head>` (spec A.7.1.2). The referenced method's hard binding has
 * no exclusion range (the hash covers the whole document).
 */
export function embedHtmlReference(html: string, url: string, newline = '\n'): string {
  const element = buildHtmlLink(url);
  const idx = headCloseIndex(html);
  return html.slice(0, idx) + element + newline + html.slice(idx);
}

function findScriptContents(html: string): string[] {
  const results: string[] = [];
  let pos = 0;
  for (;;) {
    const i = html.indexOf('<script', pos);
    if (i === -1) break;
    const gt = html.indexOf('>', i);
    if (gt === -1) break;
    const tag = html.slice(i, gt + 1);
    if (tag.includes('type="application/c2pa"')) {
      const end = html.indexOf(SCRIPT_CLOSE, gt + 1);
      if (end !== -1) {
        results.push(html.slice(gt + 1, end));
        pos = end + SCRIPT_CLOSE.length;
        continue;
      }
    }
    pos = gt + 1;
  }
  return results;
}

function findLinkTags(html: string): string[] {
  const results: string[] = [];
  let pos = 0;
  for (;;) {
    const i = html.indexOf('<link', pos);
    if (i === -1) break;
    const gt = html.indexOf('>', i);
    if (gt === -1) break;
    const tag = html.slice(i, gt + 1);
    if (tag.includes('rel="c2pa-manifest"')) results.push(tag);
    pos = gt + 1;
  }
  return results;
}

function hrefOf(tag: string): string | null {
  const marker = 'href="';
  const i = tag.indexOf(marker);
  if (i === -1) return null;
  const start = i + marker.length;
  const end = tag.indexOf('"', start);
  if (end === -1) return null;
  return tag.slice(start, end);
}

/**
 * Extract a manifest association from an HTML document (spec A.7.1.4). Returns
 * null if no association is present. Throws `HtmlError`
 * (`manifest.html.multipleManifests`) if more than one association is found.
 */
export function extractHtml(html: string): HtmlExtraction | null {
  const scripts = findScriptContents(html);
  const links = findLinkTags(html);
  const total = scripts.length + links.length;
  if (total === 0) return null;
  if (total > 1) {
    throw new HtmlError(MULTIPLE_MANIFESTS, 'More than one C2PA manifest association in HTML document');
  }
  if (scripts.length > 0) {
    return { method: 'inline', manifest: base64Decode(scripts[0].trim()), reference: null };
  }
  return { method: 'reference', manifest: null, reference: hrefOf(links[0]) };
}
