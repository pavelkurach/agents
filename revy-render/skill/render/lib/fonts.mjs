// The page's type, embedded, and a text measurer built from the same font files.
//
// Sans: Atkinson Hyperlegible Next (Braille Institute, OFL-1.1, via @fontsource-variable), one variable file per
// style. Subset: fontsource's `latin` range; other scripts fall back to the system stack.
// Mono: JetBrains Mono Nerd Font Mono (OFL-1.1), static Regular/Bold/Italic subset by fonts/subset.sh to Latin,
// punctuation, arrows, maths and box drawing. Static files because the family ships none variable; Bold also
// serves the 600-760 weights the CSS asks of code (nearest-heavier matching), and 500 falls to Regular.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { readFontMetrics } from './fontmetrics.mjs';

const require = createRequire(import.meta.url);
const SANS_PKG = '@fontsource-variable/atkinson-hyperlegible-next';
const sansFile = (file) => readFileSync(path.join(path.dirname(require.resolve(`${SANS_PKG}/package.json`)), 'files', file));
const monoFile = (style) => readFileSync(new URL(`../fonts/JetBrainsMonoNerdFontMono-${style}.subset.woff2`, import.meta.url));

/** @type {{ family: string, style: string, weight: string, data: Buffer }[]} */
const FACES = [
  { family: 'Revy Sans', style: 'normal', weight: '200 800', data: sansFile('atkinson-hyperlegible-next-latin-wght-normal.woff2') },
  { family: 'Revy Sans', style: 'italic', weight: '200 800', data: sansFile('atkinson-hyperlegible-next-latin-wght-italic.woff2') },
  { family: 'Revy Mono', style: 'normal', weight: '400', data: monoFile('Regular') },
  { family: 'Revy Mono', style: 'normal', weight: '700', data: monoFile('Bold') },
  { family: 'Revy Mono', style: 'italic', weight: '400', data: monoFile('Italic') },
];

/** @font-face rules with the fonts as data URIs; no network. */
export const FONT_CSS = FACES.map((f) => `@font-face {
  font-family: "${f.family}"; font-style: ${f.style}; font-weight: ${f.weight}; font-display: block;
  src: url(data:font/woff2;base64,${f.data.toString('base64')}) format("woff2");
}`).join('\n');

export const SANS_STACK = '"Revy Sans", "Atkinson Hyperlegible Next", system-ui, -apple-system, "Segoe UI", sans-serif';
export const MONO_STACK = '"Revy Mono", "JetBrainsMono Nerd Font Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace';

const sans = readFontMetrics(FACES[0].data);
const mono = readFontMetrics(FACES[2].data);

// Kerning is ignored, which only ever widens a box. Bold sans is measured as regular plus a margin because a
// variable font's bold advances need HVAR deltas; 8% covers this family's 700 weight. Mono is fixed-pitch in every
// weight, so bold mono needs no margin.
const BOLD = 1.08;
const FALLBACK_EM = 0.62;

const advanceEm = (metrics) => (ch) => (metrics.advance(ch.codePointAt(0)) ?? FALLBACK_EM * metrics.unitsPerEm) / metrics.unitsPerEm;
const sansEm = advanceEm(sans);
const monoEm = advanceEm(mono);

/** Pele TextMeasurer: style bits Bold=1, Italic=2, Mono=4. */
export const measurer = {
  width(text, size, style) {
    const isMono = (style & 4) !== 0;
    const w = [...text].reduce((sum, ch) => sum + (isMono ? monoEm : sansEm)(ch), 0) * size;
    return style & 1 && !isMono ? w * BOLD : w;
  },
};
