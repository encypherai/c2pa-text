/**
 * Internal helpers shared by the structured (A.9) and HTML (A.7) pipelines:
 * standard Base64 (RFC 4648 sec 4) and UTF-8 byte length. Kept dependency-free
 * and deterministic so output is byte-identical across the Rust/Python/Go SDKs.
 */

const B64_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
const utf8Encoder = new TextEncoder();

export function utf8Length(s: string): number {
  return utf8Encoder.encode(s).length;
}

/** Standard Base64 (RFC 4648 sec 4): padded, no line breaks. */
export function base64Encode(bytes: Uint8Array): string {
  let out = '';
  let i = 0;
  for (; i + 2 < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2];
    out +=
      B64_ALPHABET[(n >> 18) & 63] +
      B64_ALPHABET[(n >> 12) & 63] +
      B64_ALPHABET[(n >> 6) & 63] +
      B64_ALPHABET[n & 63];
  }
  const rem = bytes.length - i;
  if (rem === 1) {
    const n = bytes[i] << 16;
    out += B64_ALPHABET[(n >> 18) & 63] + B64_ALPHABET[(n >> 12) & 63] + '==';
  } else if (rem === 2) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8);
    out +=
      B64_ALPHABET[(n >> 18) & 63] +
      B64_ALPHABET[(n >> 12) & 63] +
      B64_ALPHABET[(n >> 6) & 63] +
      '=';
  }
  return out;
}

/** Strict standard Base64 decode. Returns null on any invalid input. */
export function base64Decode(s: string): Uint8Array | null {
  if (s.length % 4 !== 0) return null;
  if (!/^[A-Za-z0-9+/]*={0,2}$/.test(s)) return null;
  if (s.length === 0) return new Uint8Array(0);
  const rev: Record<string, number> = {};
  for (let k = 0; k < B64_ALPHABET.length; k++) rev[B64_ALPHABET[k]] = k;
  const padCount = s.endsWith('==') ? 2 : s.endsWith('=') ? 1 : 0;
  const out: number[] = [];
  for (let i = 0; i < s.length; i += 4) {
    const v0 = rev[s[i]];
    const v1 = rev[s[i + 1]];
    const c2 = s[i + 2];
    const c3 = s[i + 3];
    const v2 = c2 === '=' ? 0 : rev[c2];
    const v3 = c3 === '=' ? 0 : rev[c3];
    const n = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
    out.push((n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff);
  }
  return new Uint8Array(out.slice(0, out.length - padCount));
}
