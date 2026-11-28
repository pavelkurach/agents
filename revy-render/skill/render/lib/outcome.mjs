// Pure: the outcome a diagram label names, so an edge or end node the author left unclassed is still coloured as
// the success or the failure path it is. Whole words, case-insensitive; identifiers count by their parts
// (`ShellError` names `error`). A label naming both outcomes names neither.

/** @typedef {'ok' | 'failure'} Outcome */

const FAILURE_WORDS = new Set(['err', 'error', 'fail', 'failed', 'failure', 'reject', 'rejected', 'invalid', 'denied', 'timeout', 'panic', 'abort']);
const OK_WORDS = new Set(['ok', 'success', 'succeeded', 'pass', 'passed', 'valid', 'accepted', 'done']);
// A status code counts only when it leads the label: "retry after 200 ms" names a delay, not a 200.
const LEADING_STATUS = /^\s*([245])(?:\d\d|xx)\b/i;
const STATUS_CLASS = /^([245])xx$/;

/** @param {string} label @returns {string[]} lower-case words, identifiers split at case changes and punctuation */
const wordsOf = (label) => label
  .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
  .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2')
  .toLowerCase()
  .split(/[^a-z0-9]+/)
  .filter(Boolean);

/** @param {string} digit @returns {Outcome} */
const statusOutcome = (digit) => (digit === '2' ? 'ok' : 'failure');

/**
 * @param {string} label the label's visible text
 * @returns {Outcome | undefined}
 */
export function outcomeOf(label) {
  const words = wordsOf(label);
  const exits = words.flatMap((w, i) => (w === 'exit' && /^\d+$/.test(words[i + 1] ?? '') ? [Number(words[i + 1])] : []));
  const status = LEADING_STATUS.exec(label)?.[1];
  const classes = words.flatMap((w) => STATUS_CLASS.exec(w)?.[1] ?? []);
  const found = new Set([
    ...words.filter((w) => FAILURE_WORDS.has(w)).map(() => 'failure'),
    ...words.filter((w) => OK_WORDS.has(w)).map(() => 'ok'),
    ...exits.map((code) => (code === 0 ? 'ok' : 'failure')),
    ...[status, ...classes].filter(Boolean).map(statusOutcome),
  ]);
  return found.size === 1 ? /** @type {Outcome} */ ([...found][0]) : undefined;
}
