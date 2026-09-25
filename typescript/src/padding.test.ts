/**
 * Tests for deterministic wrapper padding.
 *
 * The specification fixes both the decomposition and the padding byte values so
 * that compliant generators emit byte-identical wrappers for the same manifest.
 */
import { encodeWrapper, encodeWrapperPadded, worstCaseWrapperByteLength, extractManifest } from './index';

const utf8Length = (s: string): number => new TextEncoder().encode(s).length;

describe('deterministic padding', () => {
  it('pads to exactly the deterministic target length', () => {
    for (const m of [0, 1, 17, 200]) {
      const manifest = new Uint8Array(m).fill(0xAB);
      const target = worstCaseWrapperByteLength(m);
      expect(utf8Length(encodeWrapperPadded(manifest, target))).toBe(target);
    }
  });

  it('uses the padding byte values fixed by the spec', () => {
    // 0x00 encodes to U+FE00 (3 UTF-8 bytes), 0x10 to U+E0100 (4 bytes).
    const manifest = new Uint8Array([0x01]);
    const base = encodeWrapper(manifest);
    const actual = utf8Length(base);
    // A gap of 12 admits four 3-byte selectors or three 4-byte ones; the
    // specified decomposition is b = 12 % 3 = 0, so four 0x00.
    expect(encodeWrapperPadded(manifest, actual + 12)).toBe(base + '\uFE00'.repeat(4));
    // A gap of 7 is b = 1, a = 1: one 0x00 then one 0x10.
    expect(encodeWrapperPadded(manifest, actual + 7)).toBe(base + '\uFE00' + '\u{E0100}');
    // A gap of 8 is b = 2, a = 0: two 0x10.
    expect(encodeWrapperPadded(manifest, actual + 8)).toBe(base + '\u{E0100}'.repeat(2));
  });

  it('ignores padding when extracting', () => {
    const manifest = new Uint8Array([0xAA, 0xBB]);
    const padded = encodeWrapperPadded(manifest, worstCaseWrapperByteLength(2));
    const result = extractManifest('Hello' + padded);
    expect(result).not.toBeNull();
    expect(Array.from(result!.manifest)).toEqual([0xAA, 0xBB]);
    expect(result!.cleanText).toBe('Hello');
  });

  it('rejects a target smaller than the unpadded wrapper', () => {
    const manifest = new Uint8Array([0x01]);
    const actual = utf8Length(encodeWrapper(manifest));
    expect(() => encodeWrapperPadded(manifest, actual - 1)).toThrow();
  });
});
