/**
 * C2PA Manifest Structural Validator.
 *
 * Provides validation utilities to help developers ensure their C2PA manifests
 * are structurally compliant before embedding them into text.
 */

// JUMBF Constants (ISO/IEC 19566-5)
const JUMBF_SUPERBOX_TYPE = new Uint8Array([0x6a, 0x75, 0x6d, 0x62]); // "jumb"
const JUMBF_DESC_TYPE = new Uint8Array([0x6a, 0x75, 0x6d, 0x64]); // "jumd"
const C2PA_MANIFEST_STORE_UUID = new Uint8Array([
  0x63, 0x32, 0x70, 0x61, 0x00, 0x11, 0x00, 0x10,
  0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
]);

// Import constants from main module
const MAGIC = new Uint8Array([0x43, 0x32, 0x50, 0x41, 0x54, 0x58, 0x54, 0x00]);
const VERSION = 1;
const HEADER_SIZE = 13;

/**
 * Registered C2PA validation status codes emitted for text manifests.
 *
 * Only members of the C2PA registered validation-status enumeration are
 * produced, so verifiers and integration partners get machine-readable interop.
 * The specific structural reason for a failure is carried in
 * `ValidationIssue.message` (the C2PA model of a coarse status code plus a
 * human-readable explanation).
 */
export enum ValidationCode {
  /** No structural issues were found. */
  Valid = "valid",

  /**
   * A `C2PATextManifestWrapper` was located but is malformed or incomplete:
   * bad magic, unsupported version, truncated/length mismatch, or an invalid
   * embedded JUMBF manifest store.
   */
  CorruptedWrapper = "manifest.text.corruptedWrapper",

  /** More than one valid `C2PATextManifestWrapper` was found in the text. */
  MultipleWrappers = "manifest.text.multipleWrappers",
}

/**
 * A single validation issue with location and details.
 */
export interface ValidationIssue {
  code: ValidationCode;
  message: string;
  offset?: number;
  context?: string;
}

/**
 * Result of manifest validation with detailed diagnostics.
 */
export interface ValidationResult {
  valid: boolean;
  issues: ValidationIssue[];
  manifestBytes?: Uint8Array;
  jumbfBytes?: Uint8Array;
  version?: number;
  declaredLength?: number;
  actualLength?: number;
}

function arraysEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function bytesToString(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => String.fromCharCode(b))
    .join("");
}

/**
 * Validate basic JUMBF box structure.
 */
export function validateJumbfStructure(
  jumbfBytes: Uint8Array,
  strict: boolean = false
): ValidationResult {
  const result: ValidationResult = {
    valid: true,
    issues: [],
    jumbfBytes,
  };

  const addIssue = (
    code: ValidationCode,
    message: string,
    offset?: number,
    context?: string
  ) => {
    result.issues.push({ code, message, offset, context });
    result.valid = false;
  };

  if (jumbfBytes.length === 0) {
    addIssue(ValidationCode.CorruptedWrapper, "JUMBF content is empty", 0);
    return result;
  }

  // Minimum JUMBF box: 8 bytes header (size + type)
  if (jumbfBytes.length < 8) {
    addIssue(
      ValidationCode.CorruptedWrapper,
      `JUMBF too short for box header: ${jumbfBytes.length} bytes, minimum 8`,
      0
    );
    return result;
  }

  // Parse first box header (Big Endian)
  const boxSize =
    (jumbfBytes[0] << 24) |
    (jumbfBytes[1] << 16) |
    (jumbfBytes[2] << 8) |
    jumbfBytes[3];
  const boxType = jumbfBytes.slice(4, 8);

  let effectiveSize: number;
  let headerSize: number;

  if (boxSize === 0) {
    // Size 0 means "extends to end of file"
    effectiveSize = jumbfBytes.length;
    headerSize = 8;
  } else if (boxSize === 1) {
    // Extended size (64-bit)
    if (jumbfBytes.length < 16) {
      addIssue(
        ValidationCode.CorruptedWrapper,
        "Extended box size declared but not enough bytes for 64-bit size field",
        0
      );
      return result;
    }
    // Read 64-bit size (simplified - read as two 32-bit values)
    const highBits =
      (jumbfBytes[8] << 24) |
      (jumbfBytes[9] << 16) |
      (jumbfBytes[10] << 8) |
      jumbfBytes[11];
    const lowBits =
      (jumbfBytes[12] << 24) |
      (jumbfBytes[13] << 16) |
      (jumbfBytes[14] << 8) |
      jumbfBytes[15];
    // For practical purposes, if highBits > 0, the file is > 4GB which is unlikely
    effectiveSize = highBits > 0 ? Number.MAX_SAFE_INTEGER : lowBits;
    headerSize = 16;
  } else if (boxSize < 8) {
    addIssue(
      ValidationCode.CorruptedWrapper,
      `Invalid box size: ${boxSize} (minimum is 8)`,
      0
    );
    return result;
  } else {
    effectiveSize = boxSize;
    headerSize = 8;
  }

  // Check if we have enough bytes
  if (jumbfBytes.length < effectiveSize) {
    addIssue(
      ValidationCode.CorruptedWrapper,
      `JUMBF truncated: declared size ${effectiveSize}, actual ${jumbfBytes.length}`,
      0
    );
    return result;
  }

  // Check for JUMBF superbox type
  if (!arraysEqual(boxType, JUMBF_SUPERBOX_TYPE)) {
    addIssue(
      ValidationCode.CorruptedWrapper,
      `Expected JUMBF superbox type 'jumb', got '${bytesToString(boxType)}'`,
      4,
      `box_type=${Array.from(boxType).map((b) => b.toString(16).padStart(2, "0")).join("")}`
    );
    return result;
  }

  if (strict) {
    // Check for description box (jumd)
    if (jumbfBytes.length < headerSize + 8) {
      addIssue(
        ValidationCode.CorruptedWrapper,
        "JUMBF superbox too short to contain description box",
        headerSize
      );
      return result;
    }

    const descType = jumbfBytes.slice(headerSize + 4, headerSize + 8);
    if (!arraysEqual(descType, JUMBF_DESC_TYPE)) {
      addIssue(
        ValidationCode.CorruptedWrapper,
        `Expected description box 'jumd', got '${bytesToString(descType)}'`,
        headerSize + 4
      );
      return result;
    }

    // Check for C2PA UUID
    const uuidOffset = headerSize + 8;
    if (jumbfBytes.length >= uuidOffset + 16) {
      const foundUuid = jumbfBytes.slice(uuidOffset, uuidOffset + 16);
      if (!arraysEqual(foundUuid, C2PA_MANIFEST_STORE_UUID)) {
        addIssue(
          ValidationCode.CorruptedWrapper,
          "Invalid C2PA manifest store UUID",
          uuidOffset,
          `expected=${Array.from(C2PA_MANIFEST_STORE_UUID).map((b) => b.toString(16).padStart(2, "0")).join("")}, found=${Array.from(foundUuid).map((b) => b.toString(16).padStart(2, "0")).join("")}`
        );
      }
    }
  }

  return result;
}

/**
 * Validate a C2PA manifest before embedding.
 *
 * This is the main validation entry point.
 */
export function validateManifest(
  manifestBytes: Uint8Array,
  validateJumbf: boolean = true,
  strict: boolean = false
): ValidationResult {
  const result: ValidationResult = {
    valid: true,
    issues: [],
    manifestBytes,
  };

  const addIssue = (
    code: ValidationCode,
    message: string,
    offset?: number,
    context?: string
  ) => {
    result.issues.push({ code, message, offset, context });
    result.valid = false;
  };

  if (manifestBytes.length === 0) {
    addIssue(ValidationCode.CorruptedWrapper, "Manifest bytes are empty");
    return result;
  }

  result.actualLength = manifestBytes.length;

  if (validateJumbf) {
    const jumbfResult = validateJumbfStructure(manifestBytes, strict);
    if (!jumbfResult.valid) {
      result.issues.push(...jumbfResult.issues);
      result.valid = false;
    }
  }

  return result;
}

/**
 * Validate a pre-encoded C2PATextManifestWrapper.
 */
export function validateWrapperBytes(wrapperBytes: Uint8Array): ValidationResult {
  const result: ValidationResult = {
    valid: true,
    issues: [],
  };

  const addIssue = (
    code: ValidationCode,
    message: string,
    offset?: number,
    context?: string
  ) => {
    result.issues.push({ code, message, offset, context });
    result.valid = false;
  };

  if (wrapperBytes.length < HEADER_SIZE) {
    addIssue(
      ValidationCode.CorruptedWrapper,
      `Wrapper too short: ${wrapperBytes.length} bytes, minimum ${HEADER_SIZE}`,
      0
    );
    return result;
  }

  // Check magic
  const magic = wrapperBytes.slice(0, 8);
  if (!arraysEqual(magic, MAGIC)) {
    addIssue(
      ValidationCode.CorruptedWrapper,
      `Invalid magic: expected 'C2PATXT\\0', got '${bytesToString(magic)}'`,
      0
    );
    return result;
  }

  // Check version
  const version = wrapperBytes[8];
  result.version = version;
  if (version !== VERSION) {
    addIssue(
      ValidationCode.CorruptedWrapper,
      `Unsupported version: ${version}, expected ${VERSION}`,
      8
    );
    return result;
  }

  // Check length
  const declaredLength =
    (wrapperBytes[9] << 24) |
    (wrapperBytes[10] << 16) |
    (wrapperBytes[11] << 8) |
    wrapperBytes[12];
  result.declaredLength = declaredLength;

  const actualJumbfLength = wrapperBytes.length - HEADER_SIZE;
  result.actualLength = actualJumbfLength;

  // Actual bytes after header must be >= declared. Trailing bytes beyond
  // manifestLength are padding (spec says decoders use manifestLength to
  // extract the manifest and ignore trailing padding).
  if (declaredLength > actualJumbfLength) {
    addIssue(
      ValidationCode.CorruptedWrapper,
      `Length mismatch: declares ${declaredLength} bytes, only ${actualJumbfLength} available (truncated)`,
      9
    );
    return result;
  }

  // Extract the declared manifest bytes (ignore trailing padding)
  const jumbfBytes = wrapperBytes.slice(HEADER_SIZE, HEADER_SIZE + declaredLength);
  result.jumbfBytes = jumbfBytes;
  result.manifestBytes = jumbfBytes;

  const jumbfResult = validateJumbfStructure(jumbfBytes, false);
  if (!jumbfResult.valid) {
    result.issues.push(...jumbfResult.issues);
    result.valid = false;
  }

  return result;
}

// Variation Selector ranges (duplicated from index.ts for self-containment)
const VS_START = 0xFE00;
const VS_END = 0xFE0F;
const VS_SUP_START = 0xE0100;
const VS_SUP_END = 0xE01EF;
const ZWNBSP = '\uFEFF';

function vsToByte(codepoint: number): number | null {
  if (codepoint >= VS_START && codepoint <= VS_END) {
    return codepoint - VS_START;
  }
  if (codepoint >= VS_SUP_START && codepoint <= VS_SUP_END) {
    return (codepoint - VS_SUP_START) + 16;
  }
  return null;
}

/**
 * Validate a text asset for C2PA text wrapper compliance.
 *
 * Scans the full text for C2PA wrappers and reports structural issues:
 * - Multiple wrappers (spec requires zero or one)
 * - Corrupted, truncated, or malformed wrappers
 * - Invalid magic bytes or unsupported version
 * - JUMBF structural issues in the embedded manifest
 *
 * This is the recommended validation entry point for text assets.
 */
export function validateText(text: string): ValidationResult {
  const result: ValidationResult = {
    valid: true,
    issues: [],
  };

  const normalized = text.normalize('NFC');

  // Scan for potential wrappers: ZWNBSP followed by variation selectors.
  interface WrapperMatch {
    charIndex: number;
    raw: Uint8Array;
  }
  const validWrappers: WrapperMatch[] = [];

  let i = 0;
  while (i < normalized.length) {
    if (normalized[i] === ZWNBSP) {
      const rawBytes: number[] = [];
      let j = i + 1;

      while (j < normalized.length) {
        const code = normalized.codePointAt(j);
        if (code === undefined) break;

        const byte = vsToByte(code);
        if (byte === null) break;

        rawBytes.push(byte);
        j += (code > 0xFFFF) ? 2 : 1;
      }

      // Check for valid C2PA header (magic bytes match).
      if (rawBytes.length >= HEADER_SIZE) {
        let magicMatch = true;
        for (let k = 0; k < 8; k++) {
          if (rawBytes[k] !== MAGIC[k]) {
            magicMatch = false;
            break;
          }
        }
        if (magicMatch) {
          validWrappers.push({
            charIndex: i,
            raw: new Uint8Array(rawBytes),
          });
        }
      }

      i = j;
      continue;
    }
    i++;
  }

  if (validWrappers.length === 0) {
    // No wrapper found is valid (wrapper is optional per spec).
    return result;
  }

  if (validWrappers.length > 1) {
    const byteOffset = new TextEncoder().encode(
      normalized.substring(0, validWrappers[1].charIndex)
    ).length;
    result.issues.push({
      code: ValidationCode.MultipleWrappers,
      message: `Found ${validWrappers.length} valid C2PA text wrappers (spec requires at most one)`,
      offset: byteOffset,
    });
    result.valid = false;
  }

  // Validate each wrapper structurally.
  for (const wrapper of validWrappers) {
    const wrapperResult = validateWrapperBytes(wrapper.raw);
    if (!wrapperResult.valid) {
      result.issues.push(...wrapperResult.issues);
      result.valid = false;
    }
  }

  return result;
}
