// Pure: a run of comma-joined code items in prose (`a.rs` (x), `b.rs` (y), `c.rs`) shown as a list, every character
// kept. A list written as one line is hard to scan; the author is told to write it as one.

const CODE = '\u0001';
const OTHER = '\u0002';
// An item: a code span (a path or code), optionally a parenthetical after it, then its separator. The last item may
// open with "and" / "or".
const ITEM = new RegExp(`^\\s*(?:(?:and|or)\\s+)?${CODE}(?:\\s*\\([^()]*\\))?\\s*[,.;:]?\\s*$`);
const MIN_ITEMS = 3;

/** A token sequence as a shape: text as itself, a code span as CODE, anything else as OTHER. */
const shape = (tokens) => tokens.map((t) => (t.type === 'text' ? t.content : t.type === 'code_inline' ? CODE : OTHER)).join('');

/**
 * Inline children cut after each comma outside parentheses; a text child holding such a comma is split in two.
 * @returns {object[][]}
 */
function pieces(children, mk) {
  const out = [[]];
  let depth = 0;
  children.forEach((c) => {
    if (c.type !== 'text') {
      out.at(-1).push(c);
      return;
    }
    let from = 0;
    [...c.content].forEach((ch, k) => {
      if (ch === '(') depth += 1;
      if (ch === ')') depth = Math.max(0, depth - 1);
      if (ch !== ',' || depth > 0) return;
      out.at(-1).push(mk('text', c.content.slice(from, k + 1), c.meta));
      out.push([]);
      from = k + 1;
    });
    if (from < c.content.length) out.at(-1).push(from === 0 ? c : mk('text', c.content.slice(from), c.meta));
  });
  return out.filter((p, k) => p.length || k < out.length - 1);
}

/**
 * The first run of at least three comma-joined code items in `children` wrapped as a list (one item per entry, its
 * separator kept), the text before it left as its lead-in.
 * @param {object[]} children
 * @param {(type: string, content: string, meta?: object) => object} mk
 * @returns {{ children: object[], items: number }} `items`: the run's length, 0 when there is none
 */
export function commaLists(children, mk) {
  const ps = pieces(children, mk);
  // The item starts at the first code span from which the rest of the piece is one item; code before it is lead-in.
  const leadOf = (p) => p.findIndex((t, k) => t.type === 'code_inline' && ITEM.test(shape(p.slice(k))));
  const opens = (p) => leadOf(p) >= 0;
  const runFrom = (s) => {
    let e = s;
    while (e + 1 < ps.length && ITEM.test(shape(ps[e + 1]))) e += 1;
    return e - s + 1;
  };
  const start = ps.findIndex((p, s) => opens(p) && runFrom(s) >= MIN_ITEMS);
  if (start < 0) return { children, items: 0 };
  const count = runFrom(start);
  const first = ps[start];
  const items = [first.slice(leadOf(first)), ...ps.slice(start + 1, start + count)];
  const html = (content) => mk('html_inline', content);
  return {
    children: [
      ...ps.slice(0, start).flat(), ...first.slice(0, leadOf(first)),
      html('<span class="comma-list" role="list">'),
      ...items.flatMap((item) => [html('<span class="comma-item" role="listitem">'), ...item, html('</span>')]),
      html('</span>'),
      ...ps.slice(start + count).flat(),
    ],
    items: count,
  };
}
