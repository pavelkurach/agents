import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { FONT_CSS, MONO_STACK, measurer } from '../lib/fonts.mjs';

const MONO = 4;
const BOLD = 1;
const theme = readFileSync(new URL('../theme.css', import.meta.url), 'utf8');

test('the mono token names the JetBrains Mono Nerd face first', () => {
  assert.match(theme, /--mono: "Revy Mono", /);
  assert.ok(MONO_STACK.startsWith('"Revy Mono", "JetBrainsMono Nerd Font Mono", '));
});

test('no stylesheet or stack names Atkinson Hyperlegible Mono', () => {
  assert.equal(`${theme}${MONO_STACK}${FONT_CSS.replace(/base64,[^)]+/g, '')}`.includes('Atkinson Hyperlegible Mono'), false);
});

test('Revy Mono is embedded as regular, bold and italic faces', () => {
  const faces = [...FONT_CSS.matchAll(/font-family: "Revy Mono"; font-style: (\w+); font-weight: (\d+)/g)].map((m) => `${m[1]} ${m[2]}`);
  assert.deepEqual(faces, ['normal 400', 'normal 700', 'italic 400']);
});

test('code ligatures stay off so -> and != read literally', () => {
  assert.match(theme, /code \{[^}]*font-variant-ligatures: none/);
});

test('mono text measures 600 units per character at 1000 per em', () => {
  assert.equal(measurer.width('abcde', 10, MONO), 30);
});

test('mono bold measures the same as mono regular', () => {
  assert.equal(measurer.width('abcde', 10, MONO | BOLD), 30);
});

test('mono arrows and box-drawing characters are measured from the font, not guessed', () => {
  assert.equal(measurer.width('→─', 10, MONO), 12);
});
