import { bundledLanguages, createHighlighter } from 'shiki';

const THEMES = { light: 'github-light', dark: 'github-dark' };

/**
 * A highlighter for the known languages among `langs`; none is loaded when there are none.
 * `highlight` returns dual-theme HTML whose colours are CSS variables (--shiki-light / --shiki-dark).
 * @param {string[]} langs
 * @returns {Promise<{ known: Set<string>, highlight: (code: string, lang: string) => string }>}
 */
export async function prepareHighlighter(langs) {
  // JSON is always loaded: values in prose and components are highlighted whenever they parse as JSON.
  const known = new Set([...langs, 'json'].filter((l) => l in bundledLanguages));
  if (!known.size) return { known, highlight: (_code, lang) => { throw new Error(`no highlighter loaded for "${lang}"`); } };
  const highlighter = await createHighlighter({ themes: Object.values(THEMES), langs: [...known] });
  return { known, highlight: (code, lang) => highlighter.codeToHtml(code, { lang, themes: THEMES, defaultColor: false }) };
}
