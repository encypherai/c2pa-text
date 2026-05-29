import * as fs from 'fs';
import * as path from 'path';

import {
  HtmlError,
  buildHtmlLink,
  buildHtmlScript,
  embedHtmlInline,
  embedHtmlReference,
  extractHtml,
} from './html';

const enc = new TextEncoder();

const HTML =
  '<!DOCTYPE html>\n<html lang="en">\n<head>\n' +
  '<meta charset="utf-8">\n<title>Example</title>\n</head>\n' +
  '<body>\n<p>Content here.</p>\n</body>\n</html>\n';

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

describe('html unit', () => {
  test('builders', () => {
    expect(buildHtmlScript(new Uint8Array([0xde, 0xad, 0xbe, 0xef]))).toBe(
      '<script type="application/c2pa">3q2+7w==</script>',
    );
    expect(buildHtmlLink('https://x/m.c2pa')).toBe(
      '<link rel="c2pa-manifest" href="https://x/m.c2pa" type="application/c2pa">',
    );
  });

  test('inline embed: exclusion + round trip', () => {
    const manifest = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
    const r = embedHtmlInline(HTML, manifest);
    const element = '<script type="application/c2pa">3q2+7w==</script>';
    const excluded = enc
      .encode(r.html)
      .slice(r.exclusionStart, r.exclusionStart + r.exclusionLength);
    expect(toHex(excluded)).toBe(toHex(enc.encode(element)));
    expect(r.html).toContain(`${element}\n</head>`);
    const x = extractHtml(r.html);
    expect(x).not.toBeNull();
    expect(x!.method).toBe('inline');
    expect(x!.manifest).toEqual(manifest);
  });

  test('inline embed without head throws', () => {
    expect(() => embedHtmlInline('<p>no head</p>', new Uint8Array([0]))).toThrow(
      expect.objectContaining({ code: 'html.noHead' }),
    );
  });

  test('reference embed: round trip', () => {
    const url = 'https://fabrikam.com/manifest.c2pa';
    const html = embedHtmlReference(HTML, url);
    const x = extractHtml(html);
    expect(x).not.toBeNull();
    expect(x!.method).toBe('reference');
    expect(x!.reference).toBe(url);
    expect(x!.manifest).toBeNull();
  });

  test('extract none', () => {
    expect(extractHtml(HTML)).toBeNull();
  });

  test('multiple manifests throws', () => {
    const r = embedHtmlInline(HTML, new Uint8Array([0]));
    const doubled = embedHtmlReference(r.html, 'https://x/m.c2pa');
    expect(() => extractHtml(doubled)).toThrow(
      expect.objectContaining({ code: 'manifest.html.multipleManifests' }),
    );
  });
});

describe('html golden vectors', () => {
  const vectors = JSON.parse(
    fs.readFileSync(path.join(__dirname, '..', '..', 'golden', 'vectors.json'), 'utf-8'),
  );

  test.each<[string, any]>(
    vectors.html_inline.map((v: any): [string, any] => [v.name, v]),
  )('html_inline %s', (_n, v: any) => {
    const r = embedHtmlInline(v.html, fromHex(v.manifest_hex), v.newline);
    expect(textHex(r.html)).toBe(v.expected_html_hex);
    expect(r.exclusionStart).toBe(v.exclusion_start);
    expect(r.exclusionLength).toBe(v.exclusion_length);
    expect(toHex(extractHtml(r.html)!.manifest!)).toBe(v.manifest_hex);
  });

  test.each<[string, any]>(
    vectors.html_reference.map((v: any): [string, any] => [v.name, v]),
  )('html_reference %s', (_n, v: any) => {
    const html = embedHtmlReference(v.html, v.url, v.newline);
    expect(textHex(html)).toBe(v.expected_html_hex);
    expect(extractHtml(html)!.reference).toBe(v.url);
  });
});
