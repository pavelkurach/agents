// The sideways-scroll affordance of every `.scroll` box, driven in headless Chrome against rendered pages.
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { after, before, test } from 'node:test';
import { pathToFileURL } from 'node:url';
import puppeteer from 'puppeteer-core';
import { findChrome, onPathIn } from '../check.mjs';
import { render } from './helpers.mjs';

const chrome = findChrome(process.env, existsSync, onPathIn(process.env, existsSync));
const inChrome = { skip: !chrome && 'no Chrome' };
let browser;
before(async () => { browser = chrome && await puppeteer.launch({ executablePath: chrome, headless: true }); });
after(async () => browser && browser.close());

async function open(md) {
  const { html } = await render(md);
  const file = path.join(mkdtempSync(path.join(tmpdir(), 'revy-cue-')), 'page.html');
  writeFileSync(file, html);
  const page = await browser.newPage();
  await page.setViewport({ width: 1280, height: 1200 });
  await page.goto(pathToFileURL(file).href, { waitUntil: 'load' });
  return page;
}

const PARTICIPANTS = Array.from({ length: 14 }, (_, i) => `P${i}`);
const WIDE_SEQUENCE = `# T\n\n\`\`\`mermaid\nsequenceDiagram\n${PARTICIPANTS.map((p) => `  participant ${p} as Service number ${p}`).join('\n')}\n${PARTICIPANTS.slice(1).map((p, i) => `  P${i}->>${p}: forwards the request`).join('\n')}\n\`\`\`\n`;
const NARROW_FLOW = '# T\n\n```mermaid\nflowchart LR\n  A[Parse] --> B[Store]\n```\n';
const cueShown = (page) => page.$eval('.diagram-frame + .scroll-cue', (c) => !c.hidden);

test('a diagram wider than its frame shows a scroll cue', inChrome, async () => {
  const page = await open(WIDE_SEQUENCE);
  assert.equal(await cueShown(page), true);
});

test('a diagram wider than its frame fades at the edge that hides more of it', inChrome, async () => {
  const page = await open(WIDE_SEQUENCE);
  assert.deepEqual(await page.$eval('.diagram-frame', (f) => [f.classList.contains('has-more-left'), f.classList.contains('has-more-right')]), [false, true]);
});

test('scrolling a wide diagram to its end hides the scroll cue', inChrome, async () => {
  const page = await open(WIDE_SEQUENCE);
  await page.$eval('.diagram-frame', (f) => { f.scrollLeft = f.scrollWidth; });
  await page.waitForFunction(() => document.querySelector('.diagram-frame').classList.contains('has-more-left'));
  assert.equal(await cueShown(page), false);
});

test('a diagram that fits its frame shows no scroll cue', inChrome, async () => {
  const page = await open(NARROW_FLOW);
  assert.equal(await cueShown(page), false);
});
