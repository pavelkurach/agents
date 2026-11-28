// The fixed marker vocabulary from revy-plan's writing.md. Colour carries meaning only here.

const STATUS_RE =
  /^(backward-compatible|feature-flag|scaffolding|breaking|one-way|reversible|unaffected|affected|existing|modified|changed|removed|added|new|pass|context|internal|n\/a)(?![\w-])/;

const slug = (word) => (word === 'n/a' ? 'na' : word);

/** Status word -> semantic kind (the colour family of its badge and of its table row's stripe). */
const STATUS_KIND = {
  new: 'new', added: 'new', pass: 'new', 'backward-compatible': 'new',
  changed: 'changed', modified: 'changed', affected: 'changed',
  removed: 'removed', breaking: 'failure', 'one-way': 'oneway',
  reversible: 'context', internal: 'context', existing: 'context', unaffected: 'context', 'n/a': 'context',
  context: 'context', 'feature-flag': 'context', scaffolding: 'context',
};

/** @returns {{word:string, cls:string, kind:string, rest:string} | undefined} */
export function statusOf(text) {
  const m = STATUS_RE.exec(text);
  return m ? { word: m[1], cls: `badge badge-${slug(m[1])}`, kind: STATUS_KIND[m[1]], rest: text.slice(m[1].length) } : undefined;
}

const MARKER_RE = /^(Risk|Accepted|Blind spot|Mitigation):(?=\s|$)/;
const MARKER_CLASS = { Risk: 'risk', Accepted: 'accepted', 'Blind spot': 'blindspot', Mitigation: 'mitigation' };

/** @returns {{label:string, cls:string} | undefined} */
export function markerOf(text) {
  const m = MARKER_RE.exec(text);
  return m ? { label: `${m[1]}:`, cls: `callout callout-${MARKER_CLASS[m[1]]}` } : undefined;
}

const NEAR_RE = /^((?:risk|accepted|blind ?spot|mitigation)s?\b[^:\n]{0,40}?):/i;

/** A warning for a line that looks like a marker but is not one exactly (it gets no colour); else undefined. */
export function nearMarkerOf(text) {
  const m = NEAR_RE.exec(text);
  if (!m || markerOf(text)) return undefined;
  return `"${m[1]}:" is not a marker, so it is not coloured; start the line with exactly one of ${Object.keys(MARKER_CLASS).map((k) => `"${k}:"`).join(', ')}`;
}
