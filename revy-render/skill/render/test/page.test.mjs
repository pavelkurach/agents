// The page script's behaviour, driven in headless Chrome against rendered pages.
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { after, before, test } from 'node:test';
import { pathToFileURL } from 'node:url';
import puppeteer from 'puppeteer-core';
import { findChrome, onPathIn } from '../check.mjs';
import { parseLinkBase } from '../lib/remote.mjs';
import { parseEvidence } from '../lib/evidence.mjs';
import { render, tempRepo } from './helpers.mjs';

const chrome = findChrome(process.env, existsSync, onPathIn(process.env, existsSync));
const inChrome = { skip: !chrome && 'no Chrome' };
let browser;
before(async () => { browser = chrome && await puppeteer.launch({ executablePath: chrome, headless: true }); });
after(async () => browser && browser.close());

/** A rendered page open in Chrome, its script run. */
async function open(md, opts) {
  const { html } = await render(md, opts);
  const file = path.join(mkdtempSync(path.join(tmpdir(), 'revy-page-')), 'page.html');
  writeFileSync(file, html);
  const page = await browser.newPage();
  await page.setViewport({ width: 1280, height: 2000 });
  await page.goto(pathToFileURL(file).href, { waitUntil: 'load' });
  return page;
}
const tip = (page) => page.$eval('#revy-tip', (t) => (t.hidden ? null : t.textContent));

const WALK = '# T\n\n```rust src/a.rs:240-243 walk\nfn a() {\n    let x = 1;\n    let y = 2;\n}\n```\n\n- L241-L242 — sets **both** up\n';

test('focusing a code chunk pops up its note', inChrome, async () => {
  const page = await open(WALK);
  await page.focus('.chunk');
  assert.equal(await tip(page), 'L241-L242 sets both up');
});

test('Escape closes a chunk note', inChrome, async () => {
  const page = await open(WALK);
  await page.focus('.chunk');
  await page.keyboard.press('Escape');
  assert.equal(await tip(page), null);
});

test('hovering a line of a chunk pops up its note', inChrome, async () => {
  const page = await open(WALK);
  await page.hover('#snip-3-L242');
  assert.equal(await tip(page), 'L241-L242 sets both up');
});

test('hovering a plain line pops up nothing', inChrome, async () => {
  const page = await open(WALK);
  await page.hover('#snip-3-L243');
  assert.equal(await tip(page), null);
});

test('an outside link opens in a new tab without a handle on this page', inChrome, async () => {
  const page = await open('# T\n\nRead [the docs](https://docs.example.org/a).\n');
  assert.deepEqual(await page.$eval('main a[href^="https"], .lead a[href^="https"]', (a) => [a.target, a.rel]), ['_blank', 'noopener']);
});

test('hovering an outside link names its domain and text', inChrome, async () => {
  const page = await open('# T\n\nRead [the docs](https://docs.example.org/a).\n');
  await page.hover('a[href^="https"]');
  assert.equal(await tip(page), 'docs.example.org — the docs');
});

test('hovering a link to a section previews its heading and first line', inChrome, async () => {
  const page = await open('# T\n\n## Flows\n\nUploads retry three times. Then they fail.\n\n## Notes\n\nSee [the flows](#flows).\n');
  await page.hover('a[href="#flows"]:not(.toc a)');
  assert.equal(await tip(page), 'FlowsUploads retry three times. Then they fail.');
});

test('hovering a link to a figure previews its caption and drawing', inChrome, async () => {
  const page = await open('# T\n\n## A\n\n```mermaid Two boxes\nflowchart LR\n  a --> b\n```\n\n## B\n\nSee [the boxes](#fig-two-boxes).\n');
  await page.hover('a[href="#fig-two-boxes"]');
  assert.deepEqual(await page.$eval('#revy-tip', (t) => [t.querySelector('.tip-title')?.textContent, Boolean(t.querySelector('svg'))]), ['Two boxes', true]);
});

test('hovering a defined id previews its definition', inChrome, async () => {
  const page = await open('# T\n\n- **R2** — the retry cap is three\n\nR2 holds.\n');
  await page.hover('a.xref');
  assert.equal(await tip(page), 'R2 — the retry cap is three');
});

test('hovering a history ref shows its tip', inChrome, async () => {
  const page = await open('# T\n\nLanded in PR #12.\n', { link: parseLinkBase('https://github.com/acme/w@1111111111111111111111111111111111111111') });
  await page.hover('a.ref');
  assert.equal(await tip(page), 'PR #12 · github.com/acme/w');
});

test('following a link into a folded section opens it', inChrome, async () => {
  const page = await open('# T\n\n## One\n\nSee [two](#two).\n\n## Two\n\nhidden\n', { look: { fold: ['Two'] } });
  await page.click('main a[href="#two"]');
  assert.equal(await page.$eval('#two', (d) => d.open), true);
});

const DIFF = '# T\n\n```diff src/a.rs\n@@ -1,2 +1,2 @@\n-let a = 1;\n+let a = one();\n keep();\n```\n\n- -L1-L2 — the old line and what stayed\n- L1 — the new call\n';

test('focusing a diff chunk on base lines pops up its note', inChrome, async () => {
  const page = await open(DIFF);
  await page.focus('.chunk');
  assert.equal(await tip(page), '-L1-L2 the old line and what stayed');
});

test('hovering a base-side note lights its base lines', inChrome, async () => {
  const page = await open(DIFF);
  await page.hover('li[data-side="base"]');
  assert.deepEqual(await page.$$eval('.line.is-lit', (ls) => ls.map((l) => l.id)), ['snip-3-B1', 'snip-3-L2']);
});

test('hovering a head-side note on a diff lights its head line', inChrome, async () => {
  const page = await open(DIFF);
  await page.hover('li:not([data-side])');
  assert.deepEqual(await page.$$eval('.line.is-lit', (ls) => ls.map((l) => l.id)), ['snip-3-L1']);
});

const TABLE = '# T\n\n| File | What changes |\n| --- | --- |\n| `src/a.rs` | the retry loop that wraps the store call and doubles its delay after every failed attempt |\n';
const widthOf = (page, n) => page.$eval(`th:nth-child(${n})`, (th) => Math.round(th.getBoundingClientRect().width));

test('each table header has a labelled resize handle', inChrome, async () => {
  const page = await open(TABLE);
  assert.deepEqual(await page.$$eval('th .col-handle', (hs) => hs.map((h) => [h.getAttribute('aria-label'), h.tabIndex])), [['Resize column File', 0], ['Resize column What changes', 0]]);
});

test('the right arrow on a resize handle widens its column', inChrome, async () => {
  const page = await open(TABLE);
  const before = await widthOf(page, 1);
  await page.focus('th:nth-child(1) .col-handle');
  await page.keyboard.press('ArrowRight');
  assert.equal(await widthOf(page, 1), before + 16);
});

test('a resized table offers to reset its widths', inChrome, async () => {
  const page = await open(TABLE);
  await page.focus('th:nth-child(1) .col-handle');
  await page.keyboard.press('ArrowRight');
  assert.equal(await page.$eval('.table-reset', (b) => b.hidden), false);
});

test('resized widths come back after a reload', inChrome, async () => {
  const page = await open(TABLE);
  await page.focus('th:nth-child(1) .col-handle');
  await page.keyboard.press('ArrowRight');
  const resized = await widthOf(page, 1);
  await page.reload({ waitUntil: 'load' });
  assert.equal(await widthOf(page, 1), resized);
});

test('resetting a resized table restores its widths', inChrome, async () => {
  const page = await open(TABLE);
  const before = await widthOf(page, 1);
  await page.focus('th:nth-child(1) .col-handle');
  await page.keyboard.press('ArrowRight');
  await page.click('.table-reset');
  await page.reload({ waitUntil: 'load' });
  assert.equal(await widthOf(page, 1), before);
});

test('double-clicking a resize handle fits the column to its content on one line', inChrome, async () => {
  const page = await open(TABLE);
  await page.click('th:nth-child(2) .col-handle', { clickCount: 2 });
  assert.equal(await page.$eval('td:nth-child(2)', (td) => {
    const s = getComputedStyle(td);
    return Math.round((td.clientHeight - parseFloat(s.paddingTop) - parseFloat(s.paddingBottom)) / parseFloat(s.lineHeight));
  }), 1);
});

test('hovering a term shows its definition with its code as code', inChrome, async () => {
  const page = await open('# T\n\nThe probe waits.\n\n## Glossary\n\n- probe: the one call to `put_object` let through\n');
  await page.hover('a.term');
  assert.equal(await page.$eval('#revy-tip', (t) => t.querySelector('code.code')?.textContent), 'put_object');
});

test('a removed diagram node that links to its code stays struck through', inChrome, async () => {
  const root = tempRepo({ files: { 'src/a.rs': 'fn a() {}\n' } });
  const page = await open('# T\n\n```mermaid Gone\nflowchart LR\n  a["src/a.rs"]:::removed --> b[kept]\n```\n', { roots: [root] });
  assert.equal(await page.$eval('a.node-link .pele-node.removed text', (t) => getComputedStyle(t).textDecorationLine), 'line-through');
});

test('a long unbroken run in one evidence entry leaves every entry inside the page on a phone', inChrome, async () => {
  const log = '## Evidence\nE1 via:read `a.md` — lints deny unwrap/expect/panic/todo/unimplemented/unreachable/indexing_slicing/dbg_macro\nE100 via:read revy-review/skill/SKILL.md::frontmatter:L1-L4 — description: "...`fix` fixes the findings and re-reviews. Invoked as /revy-review [target] [fix]." `fix` is an optional trailing word after an optional target.\n';
  const page = await open('# T\n\nSee E100.\n\n## Glossary\n\n- findings: defect claims with a severity, where, quote, breaks and fix\n- target: what the owner names to review, from nothing to a PR\n', { roots: [tempRepo({ files: { 'revy-review/skill/SKILL.md': '---\\ndescription: x\\n---\\n' }, remote: 'git@github.com:acme/w.git' })], evidence: { name: 'log.md', ...parseEvidence(log, 'log.md') } });
  await page.setViewport({ width: 390, height: 900 });
  await page.click('a.ev');
  assert.deepEqual(await page.$eval('#ev-E100', (li) => [li.closest('details').open, li.getBoundingClientRect().right <= 390]), [true, true]);
});

const TERM_NODE = '# T\n\n## Journey\n\nThe walk.\n\n## Flow\n\n```mermaid Probe path\nflowchart LR\n  a[probe] --> b[store]\n```\n\n## Glossary\n\n- probe: the one call let through, see [Journey](#journey)\n';

test('hovering a diagram node naming a term shows the term definition, as in prose', inChrome, async () => {
  const page = await open(TERM_NODE);
  await page.hover('a.node-link.node-term');
  assert.equal(await page.$eval('#revy-tip', (t) => t.textContent), 'the one call let through, see Journey');
});
