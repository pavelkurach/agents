// Classify a code span as a file path under a --repo root and build its link.
import { realpathSync } from 'node:fs';
import path from 'node:path';
import { blobUrl } from './remote.mjs';
import { symbolLineIn } from './symbols.mjs';
import { CODE_EXTENSIONS } from './autocode.mjs';

/** Split `alias:path::symbol:12-15#L3` into parts; undefined when it is not path-shaped text. */
export function parseSpan(span, aliases) {
  let rest = span;
  let alias;
  const am = /^([A-Za-z0-9_-]+):(?!\d)(.+)$/.exec(rest);
  if (am && aliases.includes(am[1])) [, alias, rest] = am;
  let lines;
  const hm = /#L(\d+)(?:-L?(\d+))?$/.exec(rest);
  if (hm) {
    lines = { start: Number(hm[1]), end: hm[2] ? Number(hm[2]) : undefined };
    rest = rest.slice(0, hm.index);
  }
  let symbol;
  const si = rest.indexOf('::');
  if (si > 0) {
    symbol = rest.slice(si + 2);
    rest = rest.slice(0, si);
  }
  const lm = /:L?(\d+)(?:-L?(\d+))?$/.exec(rest);
  if (lm && !lines) {
    lines = { start: Number(lm[1]), end: lm[2] ? Number(lm[2]) : undefined };
    rest = rest.slice(0, lm.index);
  }
  rest = rest.replace(/^\.\//, '');
  if (!/^[\w@+./-]+$/.test(rest) || rest.includes('..') || (symbol !== undefined && !symbol)) return undefined;
  return { alias, rel: rest, symbol, lines };
}

const base = (p) => p.split('/').at(-1);
const isPathShaped = (rel) => {
  const ext = base(rel).split('.').slice(1).at(-1);
  return Boolean(ext) && base(rel).indexOf('.') > 0 && (CODE_EXTENSIONS.has(ext) || (rel.includes('/') && /^[A-Za-z0-9]{1,5}$/.test(ext)));
};

const sameFile = (a, b) => a === b || a.endsWith(`/${b}`) || b.endsWith(`/${a}`);

function locate(repos, parsed) {
  const repo = parsed.alias
    ? repos.find((r) => r.alias === parsed.alias)
    : repos.find((r) => r.primary);
  if (!repo) return undefined;
  if (repo.exists(parsed.rel)) return { repo, rel: parsed.rel };
  const suffixed = repo.listFiles().filter((f) => f === parsed.rel || f.endsWith(`/${parsed.rel}`));
  return suffixed.length === 1 ? { repo, rel: suffixed[0] } : undefined;
}

/**
 * A link base (`--link`) points the primary repo's links at one forge commit: a file absent there is `local`, a file
 * whose working tree differs from it is a link with `differs`.
 * @typedef {{ remote: {kind:string, host:string, project:string}, ref: string }} LinkBase
 * @returns {{kind:'code'} | {kind:'file', href:string, cite?: string, warnings:string[], differs?: boolean}
 *   | {kind:'new'} | {kind:'missing', warnings:string[]} | {kind:'local', rel: string}}
 *   `cite` is the repository file and lines the link opens (`src/a.rs:L3`).
 */
export function classifySpan(span, { repos, addedPaths, link: base }) {
  const parsed = parseSpan(span, repos.filter((r) => !r.primary).map((r) => r.alias));
  if (!parsed) return { kind: 'code' };
  const found = locate(repos, parsed);
  if (!found) {
    if (!isPathShaped(parsed.rel)) return { kind: 'code' };
    if (addedPaths.some((a) => sameFile(a, parsed.rel))) return { kind: 'new' };
    // With no --repo there is nothing to verify against, so a link base is trusted as given.
    if (base && !repos.length && parsed.rel.includes('/')) {
      return { kind: 'file', href: blobUrl(base.remote, base.ref, parsed.rel, parsed.lines), warnings: [] };
    }
    // A bare name (`state.json`) is as often a data file the change talks about as a repository file.
    if (!parsed.rel.includes('/')) return { kind: 'code' };
    // With no --repo there is nothing to look in, so a path is shown as one but not reported.
    return { kind: 'missing', warnings: repos.length ? [`path not found in any --repo and not added in Changes: ${span}`] : [] };
  }
  if (base && found.repo.primary) return linkAtBase(found, parsed, span, base);
  return { kind: 'file', ...link(found, parsed, span) };
}

/** The line a span points at: its symbol's definition in `source`, else its written lines. */
function anchorOf(source, { rel, symbol, lines }, span, warnings) {
  if (!symbol) return lines;
  // A name followed by written lines (`SKILL.md::Tier table:L18-L22`) labels those lines; the lines are the place.
  const labelled = /(?:^|:)L(\d+)(?:-L?(\d+))?$/.exec(symbol);
  if (labelled) return { start: Number(labelled[1]), ...(labelled[2] ? { end: Number(labelled[2]) } : {}) };
  const line = symbolLineIn(rel, source, symbol);
  if (line) return { start: line };
  warnings.push(`symbol not found, linking the file only: ${span}`);
  return lines;
}

/** `src/a.rs:L3-L5`: the file and lines a link opens, as a reader would cite them. */
const citeOf = (rel, anchor) => `${rel}${anchor ? `:L${anchor.start}${anchor.end ? `-L${anchor.end}` : ''}` : ''}`;

function linkAtBase({ repo, rel }, parsed, span, base) {
  const g = repo.git;
  const relToTop = g ? path.relative(realpathSync(g.top), realpathSync(path.join(repo.root, rel))).split(path.sep).join('/') : rel;
  if (g && !g.hasAt(base.ref, relToTop)) return { kind: 'local', rel };
  // A symbol defined only in the working tree is not at the link commit: the file is linked, and marked as differing.
  const onlyLocal = Boolean(g && parsed.symbol && !symbolLineIn(rel, g.readAt(base.ref, relToTop), parsed.symbol)
    && symbolLineIn(rel, repo.read(rel), parsed.symbol));
  const warnings = [];
  const anchor = onlyLocal ? parsed.lines : anchorOf(g ? g.readAt(base.ref, relToTop) : repo.read(rel), parsed, span, warnings);
  return {
    kind: 'file', href: blobUrl(base.remote, base.ref, relToTop, anchor), cite: citeOf(rel, anchor), warnings,
    differs: Boolean(g?.differsAt(base.ref, relToTop)),
  };
}

function link({ repo, rel }, parsed, span) {
  const warnings = [];
  const anchor = anchorOf(repo.read(rel), parsed, span, warnings);
  const cite = citeOf(rel, anchor);
  const abs = path.join(repo.root, rel);
  const g = repo.git;
  if (!g?.remote) {
    if (g?.rawRemote) warnings.push(`unrecognised git host in "${g.rawRemote}", using file:// link`);
    return { href: `file://${encodeURI(abs)}`, cite, warnings };
  }
  const relToTop = path.relative(realpathSync(g.top), realpathSync(abs)).split(path.sep).join('/');
  if (!g.inHead(relToTop)) {
    warnings.push(`file is not committed at HEAD, using file:// link: ${span}`);
    return { href: `file://${encodeURI(abs)}`, cite, warnings };
  }
  return { href: blobUrl(g.remote, g.ref, relToTop, anchor), cite, warnings };
}

/** The file a code span names, without its `::symbol`, `:line` or `#L` suffix. */
export const pathOfSpan = (span) => span.replace(/(?:::.*|:L?\d+(?:-L?\d+)?|#L\d+.*)$/, '');

/** Paths a plan's Changes list or table marks `added`. */
export function addedPathsOf(planText) {
  const spans = (line) => [...line.matchAll(/`([^`]+)`/g)].map((m) => m[1]);
  return planText.split('\n').flatMap((line) => {
    const bullet = /^\s*[-*]\s+added\b/.test(line);
    const row = line.trim().startsWith('|') && line.split('|').some((c) => c.trim() === 'added');
    return bullet || row ? spans(line).map(pathOfSpan) : [];
  });
}

/**
 * A code fence's location: the first word of its info after the language, when it is a path with a line range
 * (```` ```rust src/rules.rs:240-260 caption ````). `rest` is the caption after it.
 * @returns {{ span: string, rel: string, start: number, end: number, rest: string } | undefined}
 */
export function locationOf(info) {
  const [, word = '', ...rest] = info.trim().split(/\s+/);
  const parsed = parseSpan(word, []);
  if (!parsed?.lines || parsed.symbol || !/[./]/.test(parsed.rel)) return undefined;
  const { start, end = start } = parsed.lines;
  return end < start ? undefined : { span: word, rel: parsed.rel, start, end, rest: rest.join(' ') };
}

/**
 * The file a located diff names (```` ```diff src/a.rs PR #12 E7 ````): the first word after the language when it is a
 * path without lines, an optional `PR #<n>` after it, and the words after the path as the caption (the PR included).
 * @returns {{ span: string, rel: string, pr: string | undefined, rest: string } | undefined}
 */
export function diffLocationOf(info) {
  const [, word = '', ...words] = info.trim().split(/\s+/);
  const parsed = parseSpan(word, []);
  if (!parsed || parsed.lines || parsed.symbol || !/[./]/.test(parsed.rel)) return undefined;
  const rest = words.join(' ');
  const pr = /^PR #(\d+)(?:\s+|$)/.exec(rest);
  return { span: word, rel: parsed.rel, pr: pr?.[1], rest };
}

const SOURCE_EXTENSIONS = new Set(['rs', 'py', 'ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs', 'go', 'rb', 'java', 'kt']);
const MODULE_PATH = /^([a-z_][a-z0-9_]*)((?:::[A-Za-z_]\w*)+)$/;
/** A file's module name: its stem, or its directory for `mod.rs` / `__init__.py` / `index.*`. */
const moduleOf = (file) => {
  const parts = file.split('/');
  const stem = parts.at(-1).replace(/\.[^.]+$/, '');
  return ['mod', '__init__', 'index'].includes(stem) ? parts.at(-2) : stem;
};

/**
 * A module-qualified symbol (`rules::for_languages`) as a file-qualified span (`cli/src/rules.rs::for_languages`):
 * first from a cited span of the evidence with that module and symbol, else from the one source file of the primary
 * repo named after the module. Undefined when neither settles it.
 * @param {string} span
 * @param {{ repos: object[], citations: string[] }} where
 */
export function moduleSpan(span, { repos, citations }) {
  const m = MODULE_PATH.exec(span);
  if (!m) return undefined;
  const [, mod, rest] = m;
  const last = rest.split('::').at(-1);
  const cited = citations.find((c) => {
    const [file, ...symbols] = c.split('::');
    return symbols.at(-1) === last && file.includes('.') && moduleOf(file) === mod;
  });
  if (cited) return cited;
  const repo = repos.find((r) => r.primary);
  const files = repo ? repo.listFiles().filter((f) => moduleOf(f) === mod && SOURCE_EXTENSIONS.has(f.split('.').at(-1)) && repo.exists(f)) : [];
  return files.length === 1 ? `${files[0]}${rest}` : undefined;
}

/** `classifySpan`, with a module-qualified symbol first resolved to its file. */
export function resolveSpan(span, ctx) {
  const direct = classifySpan(span, ctx);
  if (direct.kind !== 'code') return direct;
  const resolved = moduleSpan(span, ctx);
  return resolved ? classifySpan(resolved, ctx) : direct;
}
