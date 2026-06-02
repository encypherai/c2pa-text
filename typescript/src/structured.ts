/**
 * C2PA Structured Text embedding (C2PA Technical Specification 2.4, Appendix A.9).
 *
 * Associates a C2PA Manifest Store with a *structured* text asset -- source
 * code, configuration files (YAML, TOML, INI), markup (Markdown, AsciiDoc,
 * LaTeX), XML, and similar formats that support a comment or front-matter
 * convention -- using an ASCII Armour-style block (modelled on OpenPGP ASCII
 * Armor, RFC 4880 sec 6.2) delimited by:
 *
 *   -----BEGIN C2PA MANIFEST----- <manifest-reference> -----END C2PA MANIFEST-----
 *
 * The <manifest-reference> is either a URL to an external C2PA Manifest Store
 * (preferred) or a `data:application/c2pa;base64,...` URI embedding the store.
 *
 * This is a separate pipeline from the unstructured (Unicode Variation Selector)
 * method in `index.ts` (Appendix A.8). Neither pipeline is restricted to a fixed
 * set of media types: the implementer chooses which method to use for a given
 * asset. `recommendedMethod` offers an advisory mapping but is informative only.
 *
 * Wire-compatible (byte-identical output) with the Rust, Python and Go
 * `c2pa-text` structured modules for the same inputs.
 */

/** Fixed ASCII Armour-style delimiters (spec A.9.3). */
export const BEGIN_DELIMITER = '-----BEGIN C2PA MANIFEST-----';
export const END_DELIMITER = '-----END C2PA MANIFEST-----';
/** Prefix of a data: URI carrying a Base64-encoded C2PA Manifest Store. */
export const DATA_URI_PREFIX = 'data:application/c2pa;base64,';

/** Where to place the manifest block relative to the host text (spec A.9.3.1). */
export enum Placement {
  Start = 'start',
  End = 'end',
}

/** Advisory recommendation of which embedding method best fits a media type. */
export enum Method {
  Unstructured = 'unstructured',
  Structured = 'structured',
  Html = 'html',
  Svg = 'svg',
}

/** Result of embedding a structured-text manifest block. */
export interface StructuredEmbed {
  /** The host text with the manifest block inserted. */
  text: string;
  /** Byte offset of the `c2pa.hash.data` exclusion range (spec A.9.4). */
  exclusionStart: number;
  /** Byte length of the `c2pa.hash.data` exclusion range (spec A.9.4). */
  exclusionLength: number;
}

/** Result of extracting a structured-text manifest block. */
export interface StructuredExtraction {
  /** The manifest reference between the delimiters (URL or data: URI), trimmed. */
  reference: string;
  /** Decoded Manifest Store bytes, present only for a `data:` URI reference. */
  manifest: Uint8Array | null;
}

/**
 * Error from structured-text extraction. `code` is the normative C2PA
 * validation status code (spec A.9.5), e.g. `manifest.structuredText.noManifest`.
 */
export class StructuredError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.code = code;
    this.name = 'StructuredError';
  }
}

import { base64Decode, base64Encode, utf8Length } from './b64';

function countOccurrences(haystack: string, needle: string): number {
  let count = 0;
  let from = 0;
  for (;;) {
    const idx = haystack.indexOf(needle, from);
    if (idx === -1) break;
    count += 1;
    from = idx + needle.length;
  }
  return count;
}

/**
 * Build a `data:application/c2pa;base64,...` URI for a Manifest Store (A.9.3.1).
 */
export function encodeDataUri(manifestBytes: Uint8Array): string {
  return DATA_URI_PREFIX + base64Encode(manifestBytes);
}

/**
 * Decode a `data:application/c2pa;base64,...` reference into Manifest Store
 * bytes. Returns null if not such a data: URI or the Base64 payload is invalid.
 */
export function decodeDataUri(reference: string): Uint8Array | null {
  if (!reference.startsWith(DATA_URI_PREFIX)) return null;
  return base64Decode(reference.slice(DATA_URI_PREFIX.length).trim());
}

/**
 * Build a single-line manifest block (spec A.9.3.1):
 *   <prefix> -----BEGIN C2PA MANIFEST----- <reference> -----END C2PA MANIFEST----- <suffix>
 * `commentSuffix` is appended (space-separated) only when non-empty.
 */
export function buildManifestBlock(
  reference: string,
  commentPrefix: string,
  commentSuffix = '',
): string {
  let block = `${commentPrefix} ${BEGIN_DELIMITER} ${reference} ${END_DELIMITER}`;
  if (commentSuffix) block += ` ${commentSuffix}`;
  return block;
}

/**
 * Build a multi-line manifest block for placement inside host front matter
 * (spec A.9.3.2). The host front-matter fences (e.g. `---`) are not part of
 * the C2PA block and must be supplied by the caller.
 */
export function buildManifestBlockMultiline(reference: string, newline = '\n'): string {
  return `${BEGIN_DELIMITER}${newline}${reference}${newline}${END_DELIMITER}`;
}

/**
 * Embed a manifest block into structured text using the single-line comment
 * form (spec A.9.3.1) and return the resulting text together with the
 * `c2pa.hash.data` exclusion range to bind it (spec A.9.4).
 *
 * `newline` is the host line terminator -- '\n' (LF) or '\r\n' (CRLF).
 */
export function embedStructured(
  text: string,
  reference: string,
  commentPrefix: string,
  commentSuffix = '',
  placement: Placement = Placement.Start,
  newline = '\n',
): StructuredEmbed {
  const block = buildManifestBlock(reference, commentPrefix, commentSuffix);
  if (placement === Placement.Start) {
    return {
      text: block + newline + text,
      exclusionStart: 0,
      exclusionLength: utf8Length(block + newline),
    };
  }
  const start = utf8Length(text);
  return {
    text: text + newline + block,
    exclusionStart: start,
    exclusionLength: utf8Length(newline + block),
  };
}

/**
 * Extract a manifest reference from structured text (spec A.9.5). Form-agnostic:
 * the reference is whatever appears between the single pair of delimiters,
 * trimmed, so both single-line and front-matter forms are handled.
 * Throws StructuredError on noManifest / multipleReferences / emptyReference.
 */
export function extractStructured(text: string): StructuredExtraction {
  const beginCount = countOccurrences(text, BEGIN_DELIMITER);
  const endCount = countOccurrences(text, END_DELIMITER);
  if (beginCount === 0 || endCount === 0) {
    throw new StructuredError(
      'manifest.structuredText.noManifest',
      'No C2PA manifest block delimiters found',
    );
  }
  if (beginCount > 1 || endCount > 1) {
    throw new StructuredError(
      'manifest.structuredText.multipleReferences',
      'Multiple C2PA manifest blocks found',
    );
  }
  const begin = text.indexOf(BEGIN_DELIMITER) + BEGIN_DELIMITER.length;
  const end = text.indexOf(END_DELIMITER);
  if (end <= begin) {
    throw new StructuredError(
      'manifest.structuredText.noManifest',
      'Manifest block delimiters are out of order',
    );
  }
  const reference = text.slice(begin, end).trim();
  if (reference.length === 0) {
    throw new StructuredError(
      'manifest.structuredText.emptyReference',
      'Manifest reference between delimiters is empty',
    );
  }
  return { reference, manifest: decodeDataUri(reference) };
}

const RECOMMENDED: Record<string, Method> = {
  // Unstructured family (A.8). JSON and CSV have no comment/front-matter syntax,
  // so the structured method (A.9) cannot apply to them.
  'text/plain': Method.Unstructured,
  'text/markdown': Method.Unstructured,
  'text/csv': Method.Unstructured,
  'application/json': Method.Unstructured,
  // Structured family (A.9), via XML comment syntax `<!-- -->`.
  'text/xml': Method.Structured,
  'application/xml': Method.Structured,
  'application/xhtml+xml': Method.Structured,
  // Dedicated methods not implemented by this package.
  'text/html': Method.Html,
  'image/svg+xml': Method.Svg,
};

/**
 * Advisory recommendation of an embedding method for a media type, per the
 * C2PA 2.4 spec text families. Returns null for media types with no defined
 * text embedding method. Informative only.
 */
export function recommendedMethod(mime: string): Method | null {
  if (mime in RECOMMENDED) return RECOMMENDED[mime];
  if (mime.startsWith('text/')) return Method.Structured;
  return null;
}

const COMMENT_SYNTAX: Record<string, [string, string]> = {
  'text/css': ['/*', '*/'],
  'application/javascript': ['//', ''],
  'text/javascript': ['//', ''],
  'text/markdown': ['<!--', '-->'],
  'text/xml': ['<!--', '-->'],
  'application/xml': ['<!--', '-->'],
  'application/xhtml+xml': ['<!--', '-->'],
  'application/yaml': ['#', ''],
  'text/yaml': ['#', ''],
  'application/x-yaml': ['#', ''],
  'application/toml': ['#', ''],
};

/**
 * Host comment delimiters `[prefix, suffix]` used by the structured (A.9)
 * method for a media type, so the embedded armour block stays valid host
 * syntax. Returns null for media types with no comment convention (e.g.
 * application/json, text/plain, text/csv) — use the unstructured (A.8) method
 * for those. The delimiters are each language's own comment syntax; pass them
 * to embedStructured / buildManifestBlock. Distinct from recommendedMethod,
 * which advises *which* method to use.
 */
export function commentSyntax(mime: string): [string, string] | null {
  return COMMENT_SYNTAX[mime] ?? null;
}
