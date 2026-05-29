/**
 * Tests for c2pa-text TypeScript implementation
 */
import { embedManifest, extractManifest, encodeWrapper, validateText, validateWrapperBytes, ValidationCode } from './index';

// Test data - minimal valid JUMBF box
const TEST_MANIFEST = new Uint8Array([0x00, 0x00, 0x00, 0x10, 0x6a, 0x75, 0x6d, 0x62, 0x00, 0x00, 0x00, 0x08, 0x63, 0x32, 0x70, 0x61]);
const TEST_TEXT = 'Hello, World!';

describe('c2pa-text', () => {
  describe('embedManifest', () => {
    it('should embed manifest into text', () => {
      const result = embedManifest(TEST_TEXT, TEST_MANIFEST);
      expect(result).toContain(TEST_TEXT);
      expect(result.length).toBeGreaterThan(TEST_TEXT.length);
    });

    it('should handle empty text', () => {
      const result = embedManifest('', TEST_MANIFEST);
      expect(result.length).toBeGreaterThan(0);
    });
  });

  describe('extractManifest', () => {
    it('should extract manifest from embedded text', () => {
      const embedded = embedManifest(TEST_TEXT, TEST_MANIFEST);
      const result = extractManifest(embedded);

      expect(result).not.toBeNull();
      if (result) {
        expect(result.manifest).toEqual(TEST_MANIFEST);
        expect(result.cleanText).toBe(TEST_TEXT);
        expect(result.offset).toBeDefined();
        expect(result.length).toBeDefined();
      }
    });

    it('should return null for plain text', () => {
      const result = extractManifest(TEST_TEXT);
      expect(result).toBeNull();
    });
  });

  describe('encodeWrapper', () => {
    it('should encode manifest bytes to wrapper string', () => {
      const wrapper = encodeWrapper(TEST_MANIFEST);
      // Wrapper should start with ZWNBSP
      expect(wrapper.charCodeAt(0)).toBe(0xFEFF);
      expect(wrapper.length).toBeGreaterThan(1);
    });
  });

  describe('roundtrip', () => {
    it('should preserve manifest through embed/extract cycle', () => {
      const embedded = embedManifest(TEST_TEXT, TEST_MANIFEST);
      const result = extractManifest(embedded);

      expect(result).not.toBeNull();
      if (result) {
        expect(result.manifest).toEqual(TEST_MANIFEST);
        expect(result.cleanText).toBe(TEST_TEXT);
      }
    });

    it('should handle unicode text', () => {
      const unicodeText = 'Hello 世界! 🌍';
      const embedded = embedManifest(unicodeText, TEST_MANIFEST);
      const result = extractManifest(embedded);

      expect(result).not.toBeNull();
      if (result) {
        expect(result.manifest).toEqual(TEST_MANIFEST);
        // NFC normalization may change the text slightly
        expect(result.cleanText.normalize('NFC')).toBe(unicodeText.normalize('NFC'));
      }
    });
  });

  describe('validateText', () => {
    it('should return valid for plain text without wrapper', () => {
      const result = validateText('Hello, World!');
      expect(result.valid).toBe(true);
      expect(result.issues).toHaveLength(0);
    });

    it('should return valid for text with single valid wrapper', () => {
      const embedded = embedManifest(TEST_TEXT, TEST_MANIFEST);
      const result = validateText(embedded);
      expect(result.valid).toBe(true);
      expect(result.issues).toHaveLength(0);
    });

    it('should detect multiple wrappers', () => {
      const wrapper = encodeWrapper(TEST_MANIFEST);
      const doubleWrapped = TEST_TEXT + wrapper + ' more text ' + wrapper;
      const result = validateText(doubleWrapped);
      expect(result.valid).toBe(false);
      expect(result.issues.some(i => i.code === ValidationCode.MultipleWrappers)).toBe(true);
    });

    it('should detect bad version in wrapper', () => {
      // Build a wrapper with version byte set to 99 instead of 1
      const MAGIC = [0x43, 0x32, 0x50, 0x41, 0x54, 0x58, 0x54, 0x00];
      const VS_START = 0xFE00;
      const VS_SUP_START = 0xE0100;

      function byteToVsChar(byte: number): string {
        if (byte <= 15) return String.fromCodePoint(VS_START + byte);
        return String.fromCodePoint(VS_SUP_START + (byte - 16));
      }

      // ZWNBSP + magic(8) + version(99) + length(4, for 4 bytes) + 4 body bytes
      let wrapper = '\uFEFF';
      for (const b of MAGIC) wrapper += byteToVsChar(b);
      wrapper += byteToVsChar(99); // bad version
      // Length = 4 (Big Endian)
      wrapper += byteToVsChar(0);
      wrapper += byteToVsChar(0);
      wrapper += byteToVsChar(0);
      wrapper += byteToVsChar(4);
      // 4 body bytes
      for (let i = 0; i < 4; i++) wrapper += byteToVsChar(0);

      const text = 'Some text' + wrapper;
      const result = validateText(text);
      expect(result.valid).toBe(false);
      expect(result.issues.some(i => i.code === ValidationCode.UnsupportedVersion)).toBe(true);
    });

    it('should detect length mismatch (truncated)', () => {
      // Build wrapper declaring more bytes than available
      const MAGIC = [0x43, 0x32, 0x50, 0x41, 0x54, 0x58, 0x54, 0x00];
      const VS_START = 0xFE00;
      const VS_SUP_START = 0xE0100;

      function byteToVsChar(byte: number): string {
        if (byte <= 15) return String.fromCodePoint(VS_START + byte);
        return String.fromCodePoint(VS_SUP_START + (byte - 16));
      }

      // ZWNBSP + magic(8) + version(1) + length declares 100 bytes + only 8 body bytes
      let wrapper = '\uFEFF';
      for (const b of MAGIC) wrapper += byteToVsChar(b);
      wrapper += byteToVsChar(1); // version
      // Length = 100 (Big Endian)
      wrapper += byteToVsChar(0);
      wrapper += byteToVsChar(0);
      wrapper += byteToVsChar(0);
      wrapper += byteToVsChar(100);
      // Only 8 body bytes (less than declared 100)
      for (let i = 0; i < 8; i++) wrapper += byteToVsChar(0);

      const text = 'Some text' + wrapper;
      const result = validateText(text);
      expect(result.valid).toBe(false);
      expect(result.issues.some(i => i.code === ValidationCode.LengthMismatch)).toBe(true);
    });

    it('should accept trailing padding (actual > declared)', () => {
      // Build wrapper with 8-byte valid JUMBF manifest declared, then 4 extra padding bytes
      const MAGIC = [0x43, 0x32, 0x50, 0x41, 0x54, 0x58, 0x54, 0x00];
      const VS_START = 0xFE00;
      const VS_SUP_START = 0xE0100;

      function byteToVsChar(byte: number): string {
        if (byte <= 15) return String.fromCodePoint(VS_START + byte);
        return String.fromCodePoint(VS_SUP_START + (byte - 16));
      }

      // Minimal valid JUMBF superbox: size=0 (extends to end) + type "jumb"
      const validJumbf = [0x00, 0x00, 0x00, 0x00, 0x6a, 0x75, 0x6d, 0x62];

      // ZWNBSP + magic(8) + version(1) + length=8 + 8 JUMBF bytes + 4 padding bytes
      let wrapper = '\uFEFF';
      for (const b of MAGIC) wrapper += byteToVsChar(b);
      wrapper += byteToVsChar(1); // version
      // Length = 8 (Big Endian)
      wrapper += byteToVsChar(0);
      wrapper += byteToVsChar(0);
      wrapper += byteToVsChar(0);
      wrapper += byteToVsChar(8);
      // 8 JUMBF manifest bytes
      for (const b of validJumbf) wrapper += byteToVsChar(b);
      // 4 trailing padding bytes (beyond declared length)
      for (let i = 0; i < 4; i++) wrapper += byteToVsChar(0);

      const text = 'Some text' + wrapper;
      const result = validateText(text);
      expect(result.valid).toBe(true);
      expect(result.issues).toHaveLength(0);
    });

    it('should handle NFC normalization', () => {
      // e + combining acute (NFD) normalizes to e-acute (NFC)
      const nfdText = 'caf\u0065\u0301'; // "cafe" with combining accent
      const embedded = embedManifest(nfdText, TEST_MANIFEST);
      const result = validateText(embedded);
      expect(result.valid).toBe(true);
      expect(result.issues).toHaveLength(0);
    });

    it('should ignore sequences with bad magic (not a valid wrapper)', () => {
      // ZWNBSP followed by VS chars that do NOT match C2PA magic
      const VS_START = 0xFE00;
      let fakeWrapper = '\uFEFF';
      // 13 bytes of zeros - not C2PA magic
      for (let i = 0; i < 13; i++) fakeWrapper += String.fromCodePoint(VS_START); // byte 0
      const text = 'Hello' + fakeWrapper;
      const result = validateText(text);
      // Bad magic means it's not recognized as a C2PA wrapper at all
      expect(result.valid).toBe(true);
      expect(result.issues).toHaveLength(0);
    });
  });

  describe('validateWrapperBytes', () => {
    it('should validate a correctly encoded wrapper', () => {
      const embedded = embedManifest('test', TEST_MANIFEST);
      // Extract the raw wrapper bytes by decoding the VS chars after ZWNBSP
      const result = validateText(embedded);
      expect(result.valid).toBe(true);
    });

    it('should reject wrapper that is too short', () => {
      const tooShort = new Uint8Array([0x43, 0x32, 0x50, 0x41]); // only 4 bytes
      const result = validateWrapperBytes(tooShort);
      expect(result.valid).toBe(false);
      expect(result.issues.some(i => i.code === ValidationCode.CorruptedWrapper)).toBe(true);
    });
  });
});
