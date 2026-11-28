// Port of the old renderer's autocode pass: wraps only tokens whose shape is
// unambiguously code, never reinterpreting prose. A bare CamelCase word is left
// to the author.

export const CODE_EXTENSIONS = new Set([
  'rs', 'py', 'ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs', 'go', 'rb', 'java', 'kt',
  'json', 'yaml', 'yml', 'toml', 'sh', 'sql', 'txt', 'md', 'html', 'css',
]);

// A plain lowercase word plus `!` ("fast!") stays an exclamation.
const KNOWN_MACROS = new Set([
  'vec', 'format', 'print', 'println', 'eprint', 'eprintln', 'write', 'writeln',
  'panic', 'todo', 'unreachable', 'unimplemented', 'assert', 'dbg', 'matches',
  'concat', 'env',
]);

const isIdent = (c) => c !== undefined && /[A-Za-z0-9_]/.test(c);
const isPathChar = (c) => c !== undefined && /[A-Za-z0-9_./-]/.test(c);

/** Split text into [{kind:'plain'|'code', text}]; concatenation reproduces the input. */
export function segmentProse(text) {
  const chars = [...text];
  const out = [];
  let plain = '';
  let i = 0;
  while (i < chars.length) {
    const found = findToken(chars, i, i === 0 ? undefined : chars[i - 1]);
    if (found?.kind === 'code') {
      if (plain) out.push({ kind: 'plain', text: plain });
      plain = '';
      out.push({ kind: 'code', text: chars.slice(i, i + found.len).join('') });
      i += found.len;
    } else if (found?.kind === 'skip') {
      plain += chars.slice(i, i + found.len).join('');
      i += found.len;
    } else {
      plain += chars[i];
      i += 1;
    }
  }
  if (plain) out.push({ kind: 'plain', text: plain });
  return out;
}

function findToken(chars, i, prev) {
  const url = matchUrl(chars, i);
  if (url) return { kind: 'skip', len: url };
  if (prev !== undefined && (isIdent(prev) || '.:/'.includes(prev))) return undefined;
  const len =
    matchCommand(chars, i) ??
    matchPath(chars, i) ??
    matchQualified(chars, i) ??
    matchMacro(chars, i) ??
    matchSnake(chars, i);
  return len ? { kind: 'code', len } : undefined;
}

const startsWith = (chars, i, needle) => chars.slice(i, i + needle.length).join('') === needle;

function readIdent(chars, i) {
  let len = 0;
  while (isIdent(chars[i + len])) len += 1;
  return len;
}

function matchUrl(chars, i) {
  const scheme = ['https://', 'http://'].find((s) => startsWith(chars, i, s));
  if (!scheme) return undefined;
  let len = scheme.length;
  while (i + len < chars.length && !/\s/.test(chars[i + len])) len += 1;
  return len;
}

// `revy plan render`, `revy-review check`: a revy tool name plus a lowercase subcommand.
function matchCommand(chars, i) {
  const m = /^revy(?:-[a-z]+| plan) [a-z][a-z0-9-]*/.exec(chars.slice(i, i + 80).join(''));
  return m ? m[0].length : undefined;
}

function matchPath(chars, i) {
  let run = 0;
  while (isPathChar(chars[i + run])) run += 1;
  while (run > 0 && '.-/'.includes(chars[i + run - 1])) run -= 1;
  if (run === 0) return undefined;
  const text = chars.slice(i, i + run).join('');
  const dot = text.lastIndexOf('.');
  const ext = dot > 0 ? text.slice(dot + 1) : '';
  if (!ext || !/^[A-Za-z0-9]+$/.test(ext) || !CODE_EXTENSIONS.has(ext)) return undefined;
  const rest = chars.slice(i + run, i + run + 80).join('');
  const suffix = /^(?::L?\d+(?:-L?\d+)?|::[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*)/.exec(rest);
  return run + (suffix ? suffix[0].length : 0);
}

function matchQualified(chars, i) {
  let len = readIdent(chars, i);
  if (len === 0) return undefined;
  let sawSeparator = false;
  while (chars[i + len] === ':' && chars[i + len + 1] === ':') {
    const segment = readIdent(chars, i + len + 2);
    if (segment === 0) break;
    len += 2 + segment;
    sawSeparator = true;
  }
  return sawSeparator ? len : undefined;
}

function matchMacro(chars, i) {
  if (!/[a-z_]/.test(chars[i])) return undefined;
  const len = readIdent(chars, i);
  if (len < 2 || chars[i + len] !== '!') return undefined;
  const base = chars.slice(i, i + len).join('');
  if (/[A-Z]/.test(base)) return undefined;
  return base.includes('_') || KNOWN_MACROS.has(base) ? len + 1 : undefined;
}

function matchSnake(chars, i) {
  const len = readIdent(chars, i);
  if (len === 0) return undefined;
  const word = chars.slice(i, i + len).join('');
  if (word.startsWith('_') || word.endsWith('_')) return undefined;
  return word.includes('_') && /[a-z]/.test(word) ? len : undefined;
}
