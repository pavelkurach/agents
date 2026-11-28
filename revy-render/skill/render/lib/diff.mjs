// Pure: a located diff block (```` ```diff src/rules.rs E7 ````) parsed into numbered rows, and its notes' ranges
// resolved to those rows.

/** @typedef {'head' | 'base'} Side */
/**
 * @typedef {{ kind: 'hunk', text: string }
 *   | { kind: 'context' | 'add' | 'del', text: string, base: number | undefined, head: number | undefined }} Row
 *   A hunk row is its `@@` header; a code row carries the line numbers it has on each side.
 * @typedef {{ rows: Row[], spans: { base: [number, number][], head: [number, number][] } }} Diff
 *   `spans`: each hunk's first and last line number per side, for reporting.
 */

const HEADER = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@ ?(.*)$/;
// Lines a `git diff` prints before its first hunk; they name the file, which the fence already does.
const FILE_HEADER = /^(?:diff |index |--- |\+\+\+ )/;

/** Whether a fence body is a hunk-based diff: its first line that is not a file header starts `@@`. */
export const isHunked = (content) => (content.split('\n').find((l) => !FILE_HEADER.test(l)) ?? '').startsWith('@@');

/**
 * The rows of a hunked diff body, numbered per side from each `@@` header. A malformed header makes the whole diff
 * unparsed (`rows` undefined); a hunk whose lines disagree with its header's counts is still numbered, and reported.
 * @returns {{ diff: Diff | undefined, warnings: string[] }}
 */
export function parseDiff(content) {
  const rows = [];
  const spans = { base: [], head: [] };
  const warnings = [];
  let hunk;
  const close = () => {
    if (!hunk) return;
    const { header, base, head, baseCount, headCount, baseStart, headStart } = hunk;
    if (base !== baseCount || head !== headCount) {
      warnings.push(`diff hunk "${header}" has ${base} base and ${head} head lines; its header says ${baseCount} and ${headCount}`);
    }
    if (base) spans.base.push([baseStart, baseStart + base - 1]);
    if (head) spans.head.push([headStart, headStart + head - 1]);
  };
  for (const line of content.replace(/\n$/, '').split('\n')) {
    if (line.startsWith('@@')) {
      const m = HEADER.exec(line);
      if (!m) {
        return { diff: undefined, warnings: [`diff hunk header "${line}" is malformed; expected "@@ -<line>,<count> +<line>,<count> @@"; shown as a plain diff`] };
      }
      close();
      const [, b, bc = '1', h, hc = '1'] = m;
      hunk = { header: /^@@ .*? @@/.exec(line)[0], base: 0, head: 0, baseStart: Number(b), headStart: Number(h), baseCount: Number(bc), headCount: Number(hc) };
      rows.push({ kind: 'hunk', text: line });
      continue;
    }
    if (!hunk || line.startsWith('\\')) continue;
    const sign = line[0];
    const text = line.slice(1);
    const at = { base: hunk.baseStart + hunk.base, head: hunk.headStart + hunk.head };
    if (sign === '+') {
      rows.push({ kind: 'add', text, base: undefined, head: at.head });
      hunk.head += 1;
    } else if (sign === '-') {
      rows.push({ kind: 'del', text, base: at.base, head: undefined });
      hunk.base += 1;
    } else {
      rows.push({ kind: 'context', text: text ?? '', base: at.base, head: at.head });
      hunk.base += 1;
      hunk.head += 1;
    }
  }
  close();
  return { diff: { rows, spans }, warnings };
}

/** The row index holding line `n` on `side`, or -1. */
export const rowOf = (diff, side, n) => diff.rows.findIndex((r) => r.kind !== 'hunk' && r[side] === n);

/** `[[40, 46], [94, 94]]` -> `40-46, 94` */
export const spanText = (spans) => spans.map(([a, b]) => (a === b ? `${a}` : `${a}-${b}`)).join(', ');
