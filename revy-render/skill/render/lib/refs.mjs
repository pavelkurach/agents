// Pure: references in prose that point somewhere else — history refs (`commit <sha>`, `PR #n`, `issue #n`) on the
// project's forge, and ids the document defines itself (`### F3 — …`, `- **R2** — …`).

const WORD = '[\\p{L}\\p{N}_]';
export const HISTORY = new RegExp(`(?<!${WORD})(?:commit ([0-9a-f]{7,40})|PR #(\\d+)|issue #(\\d+))(?!${WORD})`, 'gu');

/** @typedef {{ kind: 'commit' | 'pr' | 'issue', id: string, text: string }} HistoryRef */

/** @returns {HistoryRef} the ref `text` (one `HISTORY` match) names */
export function historyRef(text) {
  const [, sha, pr, issue] = new RegExp(HISTORY.source, 'u').exec(text);
  if (sha) return { kind: 'commit', id: sha, text };
  return pr ? { kind: 'pr', id: pr, text } : { kind: 'issue', id: issue, text };
}

const FORGE_PATHS = {
  github: { commit: 'commit', pr: 'pull', issue: 'issues' },
  gitlab: { commit: '-/commit', pr: '-/merge_requests', issue: '-/issues' },
  bitbucket: { commit: 'commits', pr: 'pull-requests', issue: 'issues' },
};

/** @param {{ kind: 'github' | 'gitlab' | 'bitbucket', host: string, project: string }} remote @param {HistoryRef} ref */
export const historyUrl = (remote, ref) => `https://${remote.host}/${remote.project}/${FORGE_PATHS[remote.kind][ref.kind]}/${ref.id}`;

const PR_FILES = { github: 'pull/{n}/files', gitlab: '-/merge_requests/{n}/diffs', bitbucket: 'pull-requests/{n}/diff' };

/** The files view of PR `n` on the forge. */
export const prFilesUrl = (remote, n) => `https://${remote.host}/${remote.project}/${PR_FILES[remote.kind].replace('{n}', n)}`;

/**
 * The ledger entries that mention `ref`: a commit by its first seven digits, a PR or issue by `PR #n` / `issue #n` or
 * its forge path.
 * @param {Iterable<import('./evidence.mjs').EvidenceEntry>} entries
 */
export function ledgerMentions(entries, ref) {
  const said = {
    commit: new RegExp(`${ref.id.slice(0, 7)}`, 'i'),
    pr: new RegExp(`(?:PR\\s*#|pull/|merge_requests/|pull-requests/)${ref.id}(?!\\d)`, 'i'),
    issue: new RegExp(`(?:issue\\s*#|issues/)${ref.id}(?!\\d)`, 'i'),
  }[ref.kind];
  return [...entries].filter((e) => said.test([e.citation, e.what, e.backs ?? '', ...e.markers].join(' ')));
}

const DEFINED = /^([A-Z]{1,3}\d{1,3})(?=\s*(?:[—–:-]|$))/;
const BARE_ID = /^[A-Z]{1,3}\d{1,3}$/;

/**
 * Ids the document defines: a heading opening `F3 — ` or a paragraph, list item or table cell opening with a bold id
 * (`**R2**`). Each definition's block gets the id `def-<id>`, and its own id text is marked so it does not link to
 * itself. A second definition of an id is reported and the first kept.
 * @returns {{ defined: Map<string, string>, warnings: { text: string, line: number }[] }} id -> anchor id
 */
export function definedIds(tokens) {
  const defined = new Map();
  const warnings = [];
  tokens.forEach((t, i) => {
    if (t.type !== 'inline' || !t.children?.length) return;
    const open = tokens[i - 1];
    const [first, text, close] = t.children;
    const heading = open.type === 'heading_open' && open.tag !== 'h1' ? DEFINED.exec(first.type === 'text' ? first.content : '')?.[1] : undefined;
    const bold = !heading && first.type === 'strong_open' && text?.type === 'text' && close?.type === 'strong_close' && BARE_ID.test(text.content) ? text.content : undefined;
    const id = heading ?? bold;
    if (!id) return;
    const line = (open.map?.[0] ?? 0) + 1;
    if (defined.has(id)) {
      warnings.push({ text: `id ${id} is defined twice; links go to the first`, line });
      return;
    }
    const block = bold && open.type === 'paragraph_open' && tokens[i - 2]?.type === 'list_item_open' ? tokens[i - 2] : open;
    block.attrSet('id', `def-${id}`);
    if (bold) text.meta = { ...text.meta, defines: true };
    defined.set(id, `def-${id}`);
  });
  return { defined, warnings };
}

/** One whole-word pattern for the defined ids, or undefined without any. */
export const definedPattern = (defined) => (defined.size
  ? new RegExp(`(?<!${WORD}|${WORD}-)(${[...defined.keys()].sort((a, b) => b.length - a.length).join('|')})(?!${WORD}|-${WORD})`, 'gu')
  : undefined);

const slug = (s) => s.toLowerCase().replace(/<[^>]*>/g, '').replace(/&[a-z#0-9]+;/g, ' ').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');

/**
 * The page with each wanted `fig-<words>` id given to the first figure whose caption opens with those words (whole
 * words, any case, punctuation ignored). Figures nobody links to stay without an id.
 * @param {string} html @param {string[]} wanted ids, `fig-` ones are considered
 */
export function anchorFigures(html, wanted) {
  const asks = [...new Set(wanted.filter((id) => id.startsWith('fig-') && id.length > 4))];
  if (!asks.length) return html;
  const placed = new Set();
  return html.replace(/<figure (class="[^"]*"[^>]*>)<figcaption>([^]*?)<\/figcaption>/g, (whole, rest, caption) => {
    const words = `${slug(caption)}-`;
    const id = asks.find((a) => !placed.has(a) && words.startsWith(`${a.slice(4)}-`));
    if (!id) return whole;
    placed.add(id);
    return `<figure id="${id}" ${rest}<figcaption>${caption}</figcaption>`;
  });
}
