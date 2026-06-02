import * as fs from 'fs';
import * as path from 'path';

import { embedManifest, extractManifest } from './index';
import {
  BEGIN_DELIMITER,
  END_DELIMITER,
  Method,
  Placement,
  StructuredError,
  buildManifestBlock,
  buildManifestBlockMultiline,
  commentSyntax,
  decodeDataUri,
  embedStructured,
  encodeDataUri,
  extractStructured,
  recommendedMethod,
} from './structured';

const enc = new TextEncoder();

function toHex(u: Uint8Array): string {
  return Array.from(u)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

function fromHex(s: string): Uint8Array {
  const a = new Uint8Array(s.length / 2);
  for (let i = 0; i < a.length; i++) a[i] = parseInt(s.substr(i * 2, 2), 16);
  return a;
}

function textHex(s: string): string {
  return toHex(enc.encode(s));
}

describe('structured unit', () => {
  test('data: URI round trip', () => {
    const data = new Uint8Array([0x01, 0x02, 0x03, 0xff, 0x00]);
    const uri = encodeDataUri(data);
    expect(uri).toBe('data:application/c2pa;base64,AQID/wA=');
    expect(decodeDataUri(uri)).toEqual(data);
    expect(decodeDataUri('https://example.com/m.c2pa')).toBeNull();
    expect(decodeDataUri('data:application/c2pa;base64,!!!!')).toBeNull();
  });

  test('single-line block format', () => {
    expect(buildManifestBlock('https://x/m.c2pa', '#')).toBe(
      '# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----',
    );
    expect(buildManifestBlock('https://x/m.c2pa', '<!--', '-->')).toBe(
      '<!-- -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST----- -->',
    );
  });

  test('multiline block format', () => {
    expect(buildManifestBlockMultiline('https://x/m.c2pa')).toBe(
      '-----BEGIN C2PA MANIFEST-----\nhttps://x/m.c2pa\n-----END C2PA MANIFEST-----',
    );
  });

  test('embed at start: exclusion + round trip', () => {
    const block =
      '# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----';
    const r = embedStructured('body line 1\nbody line 2\n', 'https://x/m.c2pa', '#');
    expect(r.text).toBe(`${block}\nbody line 1\nbody line 2\n`);
    expect(r.exclusionStart).toBe(0);
    expect(r.exclusionLength).toBe(enc.encode(`${block}\n`).length);
    const x = extractStructured(r.text);
    expect(x.reference).toBe('https://x/m.c2pa');
    expect(x.manifest).toBeNull();
  });

  test('embed at end: exclusion starts at preceding newline', () => {
    const text = "#!/usr/bin/env python\nprint('hi')\n";
    const block =
      '# -----BEGIN C2PA MANIFEST----- https://x/m.c2pa -----END C2PA MANIFEST-----';
    const r = embedStructured(text, 'https://x/m.c2pa', '#', '', Placement.End);
    expect(r.text).toBe(`${text}\n${block}`);
    expect(r.exclusionStart).toBe(enc.encode(text).length);
    expect(r.exclusionLength).toBe(enc.encode(`\n${block}`).length);
  });

  test('embed and extract data: URI', () => {
    const manifest = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
    const uri = encodeDataUri(manifest);
    const r = embedStructured('doc\n', uri, '//');
    const x = extractStructured(r.text);
    expect(x.reference).toBe(uri);
    expect(x.manifest).toEqual(manifest);
  });

  test('extract errors carry status codes', () => {
    expect(() => extractStructured('nothing here')).toThrow(
      expect.objectContaining({ code: 'manifest.structuredText.noManifest' }),
    );
    expect(() => extractStructured(`# ${BEGIN_DELIMITER} https://x`)).toThrow(
      expect.objectContaining({ code: 'manifest.structuredText.noManifest' }),
    );
    expect(() => extractStructured(`# ${BEGIN_DELIMITER}   ${END_DELIMITER}`)).toThrow(
      expect.objectContaining({ code: 'manifest.structuredText.emptyReference' }),
    );
    const two = `# ${BEGIN_DELIMITER} a ${END_DELIMITER}\n# ${BEGIN_DELIMITER} b ${END_DELIMITER}`;
    expect(() => extractStructured(two)).toThrow(
      expect.objectContaining({ code: 'manifest.structuredText.multipleReferences' }),
    );
  });

  test('front matter form extracts', () => {
    const doc =
      '---\n-----BEGIN C2PA MANIFEST-----\nhttps://x/m.c2pa\n-----END C2PA MANIFEST-----\ntitle: Doc\n---\nbody\n';
    expect(extractStructured(doc).reference).toBe('https://x/m.c2pa');
  });

  test('recommended method', () => {
    expect(recommendedMethod('text/plain')).toBe(Method.Unstructured);
    expect(recommendedMethod('application/json')).toBe(Method.Unstructured);
    expect(recommendedMethod('text/csv')).toBe(Method.Unstructured);
    expect(recommendedMethod('application/xml')).toBe(Method.Structured);
    expect(recommendedMethod('text/x-python')).toBe(Method.Structured);
    expect(recommendedMethod('text/html')).toBe(Method.Html);
    expect(recommendedMethod('image/svg+xml')).toBe(Method.Svg);
    expect(recommendedMethod('image/jpeg')).toBeNull();
  });

  test('comment syntax', () => {
    expect(commentSyntax('text/css')).toEqual(['/*', '*/']);
    expect(commentSyntax('application/javascript')).toEqual(['//', '']);
    expect(commentSyntax('application/xml')).toEqual(['<!--', '-->']);
    expect(commentSyntax('text/markdown')).toEqual(['<!--', '-->']);
    expect(commentSyntax('application/yaml')).toEqual(['#', '']);
    expect(commentSyntax('application/toml')).toEqual(['#', '']);
    expect(commentSyntax('application/json')).toBeNull();
    expect(commentSyntax('text/plain')).toBeNull();
    expect(commentSyntax('image/jpeg')).toBeNull();
    const cs = commentSyntax('text/css')!;
    expect(buildManifestBlock('data:application/c2pa;base64,AA==', cs[0], cs[1])).toBe(
      '/* -----BEGIN C2PA MANIFEST----- data:application/c2pa;base64,AA== -----END C2PA MANIFEST----- */',
    );
  });
});

describe('golden vectors (cross-language parity)', () => {
  const vectors = JSON.parse(
    fs.readFileSync(path.join(__dirname, '..', '..', 'golden', 'vectors.json'), 'utf-8'),
  );

  test.each<[string, any]>(vectors.data_uri.map((v: any): [string, any] => [v.name, v]))('data_uri %s', (_n, v: any) => {
    expect(encodeDataUri(fromHex(v.manifest_hex))).toBe(v.expected_uri);
    expect(toHex(decodeDataUri(v.expected_uri)!)).toBe(v.manifest_hex);
  });

  test.each<[string, any]>(vectors.structured_block.map((v: any): [string, any] => [v.name, v]))(
    'structured_block %s',
    (_n, v: any) => {
      expect(buildManifestBlock(v.reference, v.comment_prefix, v.comment_suffix)).toBe(
        v.expected_block,
      );
    },
  );

  test.each<[string, any]>(vectors.structured_multiline.map((v: any): [string, any] => [v.name, v]))(
    'structured_multiline %s',
    (_n, v: any) => {
      expect(buildManifestBlockMultiline(v.reference, v.newline)).toBe(v.expected_block);
    },
  );

  test.each<[string, any]>(vectors.structured_embed.map((v: any): [string, any] => [v.name, v]))(
    'structured_embed %s',
    (_n, v: any) => {
      const placement = v.placement === 'end' ? Placement.End : Placement.Start;
      const r = embedStructured(
        v.text,
        v.reference,
        v.comment_prefix,
        v.comment_suffix,
        placement,
        v.newline,
      );
      expect(textHex(r.text)).toBe(v.expected_text_hex);
      expect(r.exclusionStart).toBe(v.exclusion_start);
      expect(r.exclusionLength).toBe(v.exclusion_length);
      expect(extractStructured(r.text).reference).toBe(v.reference);
    },
  );

  test.each<[string, any]>(vectors.unstructured_embed.map((v: any): [string, any] => [v.name, v]))(
    'unstructured_embed %s',
    (_n, v: any) => {
      const manifest = fromHex(v.manifest_hex);
      expect(textHex(embedManifest(v.text, manifest))).toBe(v.expected_embed_hex);
      const extracted = extractManifest(embedManifest(v.text, manifest));
      expect(extracted).not.toBeNull();
      expect(toHex(extracted!.manifest)).toBe(v.manifest_hex);
    },
  );
});
