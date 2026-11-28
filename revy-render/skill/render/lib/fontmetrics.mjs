// Pure: advance widths read from a WOFF2 font, so diagram labels are
// measured with the exact font the page embeds (Pele lays out boxes from these widths).
import { brotliDecompressSync } from 'node:zlib';

const KNOWN_TAGS = [
  'cmap', 'head', 'hhea', 'hmtx', 'maxp', 'name', 'OS/2', 'post', 'cvt ', 'fpgm', 'glyf', 'loca', 'prep', 'CFF ',
  'VORG', 'EBDT', 'EBLC', 'gasp', 'hdmx', 'kern', 'LTSH', 'PCLT', 'VDMX', 'vhea', 'vmtx', 'BASE', 'GDEF', 'GPOS',
  'GSUB', 'EBSC', 'JSTF', 'MATH', 'CBDT', 'CBLC', 'COLR', 'CPAL', 'SVG ', 'sbix', 'acnt', 'avar', 'bdat', 'bloc',
  'bsln', 'cvar', 'fdsc', 'feat', 'fmtx', 'fvar', 'gvar', 'hsty', 'just', 'lcar', 'mort', 'morx', 'opbd', 'prop',
  'trak', 'Zapf', 'Silf', 'Glat', 'Gloc', 'Feat', 'Sill',
];

function readBase128(buf, at) {
  let value = 0;
  for (let i = 0; i < 5; i += 1) {
    const byte = buf[at.pos++];
    value = value * 128 + (byte & 0x7f);
    if (!(byte & 0x80)) return value;
  }
  throw new Error('woff2: UIntBase128 longer than 5 bytes');
}

/** Decompressed sfnt tables of a WOFF2 file: Map tag -> {table, transformed}. */
function woff2Tables(buf) {
  if (buf.toString('latin1', 0, 4) !== 'wOF2') throw new Error('not a WOFF2 font');
  const numTables = buf.readUInt16BE(12);
  const compressedSize = buf.readUInt32BE(20);
  const at = { pos: 48 };
  const dir = Array.from({ length: numTables }, () => {
    const flags = buf[at.pos++];
    const tag = (flags & 0x3f) === 63 ? buf.toString('latin1', at.pos, (at.pos += 4)) : KNOWN_TAGS[flags & 0x3f];
    const version = flags >> 6;
    const origLength = readBase128(buf, at);
    const transformed = tag === 'glyf' || tag === 'loca' ? version === 0 : version !== 0;
    const length = transformed ? readBase128(buf, at) : origLength;
    return { tag, length, transformed };
  });
  const data = brotliDecompressSync(buf.subarray(at.pos, at.pos + compressedSize));
  let offset = 0;
  return new Map(dir.map(({ tag, length, transformed }) => {
    const table = data.subarray(offset, offset + length);
    offset += length;
    return [tag, { table, transformed }];
  }));
}

function cmapOf(cmap) {
  const n = cmap.readUInt16BE(2);
  const subtables = Array.from({ length: n }, (_, i) => ({
    platform: cmap.readUInt16BE(4 + i * 8),
    encoding: cmap.readUInt16BE(6 + i * 8),
    offset: cmap.readUInt32BE(8 + i * 8),
  }));
  const map = new Map();
  const f12 = subtables.find((s) => s.platform === 3 && s.encoding === 10 && cmap.readUInt16BE(s.offset) === 12);
  if (f12) {
    const groups = cmap.readUInt32BE(f12.offset + 12);
    for (let g = 0; g < groups; g += 1) {
      const p = f12.offset + 16 + g * 12;
      const [start, end, glyph] = [cmap.readUInt32BE(p), cmap.readUInt32BE(p + 4), cmap.readUInt32BE(p + 8)];
      for (let c = start; c <= end; c += 1) map.set(c, glyph + c - start);
    }
    return map;
  }
  const f4 = subtables.find((s) => s.platform === 3 && (s.encoding === 1 || s.encoding === 0) && cmap.readUInt16BE(s.offset) === 4);
  if (!f4) throw new Error('font has no Unicode cmap (format 4 or 12)');
  const o = f4.offset;
  const segs = cmap.readUInt16BE(o + 6) / 2;
  const ends = o + 14;
  const starts = ends + segs * 2 + 2;
  const deltas = starts + segs * 2;
  const ranges = deltas + segs * 2;
  for (let s = 0; s < segs; s += 1) {
    const [end, start] = [cmap.readUInt16BE(ends + s * 2), cmap.readUInt16BE(starts + s * 2)];
    const delta = cmap.readInt16BE(deltas + s * 2);
    const rangeOffset = cmap.readUInt16BE(ranges + s * 2);
    for (let c = start; c <= end && c !== 0xffff; c += 1) {
      const glyph = rangeOffset === 0
        ? (c + delta) & 0xffff
        : cmap.readUInt16BE(ranges + s * 2 + rangeOffset + (c - start) * 2);
      if (glyph) map.set(c, rangeOffset === 0 ? glyph : (glyph + delta) & 0xffff);
    }
  }
  return map;
}

/**
 * @returns {{ unitsPerEm:number, advance:(codePoint:number)=>number|undefined }}
 * `advance` is in font units; undefined when the font has no glyph for the code point.
 */
export function readFontMetrics(woff2) {
  const tables = woff2Tables(woff2);
  const need = (tag) => tables.get(tag)?.table ?? (() => { throw new Error(`font has no ${tag} table`); })();
  const unitsPerEm = need('head').readUInt16BE(18);
  const metrics = need('hhea').readUInt16BE(34);
  const hmtx = need('hmtx');
  const cmap = cmapOf(need('cmap'));
  // A transformed hmtx (WOFF2 version 1) still starts with the advance widths.
  const stride = tables.get('hmtx').transformed ? 2 : 4;
  const width = (glyph) => hmtx.readUInt16BE(Math.min(glyph, metrics - 1) * stride);
  return {
    unitsPerEm,
    advance: (cp) => (cmap.has(cp) ? width(cmap.get(cp)) : undefined),
  };
}
