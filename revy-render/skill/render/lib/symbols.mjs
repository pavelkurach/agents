// Pure: find the 1-based line where a symbol is defined, via a simple
// language-aware regex. Not a parser; first definition-looking line wins.

const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const DEFINERS = [
  'fn', 'struct', 'enum', 'union', 'impl', 'trait', 'mod', 'type', 'const', 'static',
  'macro_rules!', 'def', 'class', 'function', 'interface', 'namespace', 'let', 'var', 'func', 'object', 'record',
];

export function findSymbolLine(source, symbol) {
  const name = escapeRe(symbol.split('::').filter(Boolean).at(-1) ?? symbol);
  const definer = new RegExp(
    `\\b(?:${DEFINERS.map(escapeRe).join('|')})\\s+(?:<[^>]*>\\s*)?(?:\\([^)]*\\)\\s*)?${name}\\b`,
  );
  const assigned = new RegExp(`^\\s*(?:export\\s+)?(?:[A-Za-z_$][\\w$]*\\s+)*${name}\\s*(?::[^=]*)?=\\s*(?:async\\s*)?(?:function\\b|\\()`);
  const method = new RegExp(`^\\s*(?:(?:pub|public|private|protected|static|async|export)\\s+)*${name}\\s*\\([^)]*\\)\\s*(?:\\{|:)`);
  const lines = source.split('\n');
  const index = lines.findIndex((l) => definer.test(l) || assigned.test(l) || method.test(l));
  return index === -1 ? undefined : index + 1;
}

const HEADING = /^#{1,6}\s+(.*?)\s*#*\s*$/;

/**
 * The line a symbol names in a markdown file: `frontmatter` is the front matter block, any other name the first
 * heading whose text starts with it (any case).
 * @returns {number | undefined}
 */
export function findHeadingLine(source, symbol) {
  const lines = source.split('\n');
  if (symbol.toLowerCase() === 'frontmatter') return lines[0] === '---' ? 1 : undefined;
  const want = symbol.toLowerCase();
  const index = lines.findIndex((l) => HEADING.exec(l)?.[1].toLowerCase().startsWith(want));
  return index === -1 ? undefined : index + 1;
}

/** The line `symbol` names in the file `rel`: a heading in markdown, a definition in code. */
export const symbolLineIn = (rel, source, symbol) => (/\.(?:md|markdown)$/i.test(rel) ? findHeadingLine(source, symbol) : findSymbolLine(source, symbol));
