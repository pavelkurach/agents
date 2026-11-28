// Pure: the evidence ledger a document cites by id (`E3`), read from the lines under an `Evidence` heading of a log:
// `E<n> via:<marker> <citation> — <what it showed>`, optionally ending `| backs: <statement>`.
export class EvidenceError extends Error {}

/**
 * @typedef {{ id: string, markers: string[], citation: string, what: string, backs: string | undefined, line: number }} EvidenceEntry
 */

const HEADING = /^(#{1,6})\s+(.*)$/;
const ENTRY = /^\s*(?:[-*]\s+)?(E\d+)\b\s*[·:|]?\s*(.*)$/;
// Leading `via:read`, `via:read+via:grep`, `via:read ·`, possibly backticked — the markers, before the citation.
const MARKERS = /^(`?via:[\w-]+`?(?:\s*[+,]\s*`?via:[\w-]+`?)*)\s*[·|]?\s*/;
const SEPARATOR = /\s+[—–]\s+/;

/**
 * @param {string} text the log
 * @param {string} file its name, for messages
 * @returns {{ entries: Map<string, EvidenceEntry>, warnings: string[] }}
 */
export function parseEvidence(text, file) {
  const lines = text.split('\n');
  const start = lines.findIndex((l) => {
    const h = HEADING.exec(l);
    return h && /^evidence\b/i.test(h[2].trim());
  });
  if (start === -1) throw new EvidenceError(`--evidence ${file} has no "Evidence" heading; its entries go under one, e.g. "## Evidence"`);
  const level = HEADING.exec(lines[start])[1].length;
  const endAt = lines.findIndex((l, i) => i > start && HEADING.exec(l)?.[1].length <= level);
  const body = lines.slice(start + 1, endAt === -1 ? lines.length : endAt);
  const entries = new Map();
  const warnings = [];
  body.forEach((raw, i) => {
    const m = ENTRY.exec(raw);
    if (!m) return;
    const line = start + 2 + i;
    const [, id, after] = m;
    if (entries.has(id)) {
      warnings.push(`evidence ${id} is listed twice; keeping the first — ${file}:${line}`);
      return;
    }
    entries.set(id, { id, line, ...fieldsOf(after) });
  });
  if (!entries.size) warnings.push(`--evidence ${file} lists no entries (lines "E<n> via:<marker> <citation> — <what>") under its Evidence heading — ${file}:${start + 1}`);
  return { entries, warnings };
}

/** The parts of an entry after its id. A line without ` — ` has no separate citation: all of it is what it showed. */
function fieldsOf(after) {
  const mk = MARKERS.exec(after);
  const markers = mk ? [...mk[1].matchAll(/via:([\w-]+)/g)].map((x) => x[1]) : [];
  const rest = mk ? after.slice(mk[0].length) : after;
  const bar = rest.search(/\s+\|\s+backs:\s*/);
  const [claim, backs] = bar === -1 ? [rest, undefined] : [rest.slice(0, bar), rest.slice(bar).replace(/^\s+\|\s+backs:\s*/, '')];
  const sep = SEPARATOR.exec(claim);
  const citation = sep ? claim.slice(0, sep.index).replace(/[\s·|]+$/, '') : '';
  const what = sep ? claim.slice(sep.index + sep[0].length) : claim;
  return { markers, citation: citation.trim(), what: what.trim(), backs: backs?.trim() };
}
