// The gallery exercises every feature at once; these tests hold the whole-document contracts against it.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { COMPONENT_LANGS } from '../lib/components/index.mjs';
import { DIAGRAM_LANGS } from '../lib/diagram.mjs';
import { FIGURE_LANGS } from '../lib/figure.mjs';
import { createMarkdown } from '../lib/markdown.mjs';
import { render } from './helpers.mjs';

const GALLERY = readFileSync(new URL('./fixtures/gallery.md', import.meta.url), 'utf8');
const words = (s) => (s.match(/[\p{L}\p{N}_]+/gu) ?? []).join(' ');
const DRAWN = new Set([...COMPONENT_LANGS, ...DIAGRAM_LANGS, ...FIGURE_LANGS]);

/** Every prose text, code block body and fence caption of the gallery, as its words. */
function sourceTexts() {
  const tokens = createMarkdown().parse(GALLERY, { ctx: { warn: () => {}, fact: () => {} } });
  const fenceLang = (t) => t.info.trim().split(/\s+/)[0].toLowerCase();
  // A footnote reference shows as its number, so its label is not text.
  const prose = tokens.filter((t) => t.type === 'inline').map((t) => t.content.replace(/\[\^[^\]]+\]/g, ''));
  const code = tokens.filter((t) => t.type === 'fence' && !DRAWN.has(fenceLang(t))).map((t) => t.content);
  const captions = tokens.filter((t) => t.type === 'fence').map((t) => t.info.trim().split(/\s+/).slice(1).join(' '));
  return [...prose, ...code, ...captions].map(words).filter(Boolean);
}

test('every prose text, code block and fence caption of the gallery appears in the page', async () => {
  const { html } = await render(GALLERY);
  // Tags become spaces so a term and its definition stay two words; the "new file" chip is the renderer's own word.
  const page = words(html.replace(/<style[\s\S]*?<\/style>|<svg[\s\S]*?<\/svg>|<span class="chip[^"]*">[^<]*<\/span>/g, ' ').replace(/<[^>]+>/g, ' ')
    .replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCodePoint(parseInt(h, 16))).replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(Number(d)))
    .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&'));
  assert.deepEqual(sourceTexts().filter((text) => !page.includes(text)), []);
});

test('the rendered gallery makes no network request', async () => {
  const { html } = await render(GALLERY);
  assert.deepEqual(html.match(/(?:src|href|xlink:href|srcset)\s*=\s*["']?(?:https?:)?\/\/[^"'\s>]*|url\(\s*["']?(?:https?:)?\/\/|@import/g) ?? [], []);
});
