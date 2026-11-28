import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { checkPage, findChrome, mergeProblems, onPathIn, parseCli, reportLines, sheetLayout, tileRanges, verdictLine } from '../check.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const fixture = (name) => path.join(here, 'fixtures', 'check', name);
const chrome = findChrome(process.env, existsSync, onPathIn(process.env, existsSync));
const inChrome = { skip: !chrome && 'no Chrome' };
const pngHeight = (file) => readFileSync(file).readUInt32BE(20);
const sheetIn = () => path.join(mkdtempSync(path.join(tmpdir(), 'revy-check-')), 'sheet.png');
const linesOf = async (name) => (await checkPage(fixture(name), { sheet: sheetIn(), chrome })).lines;

test('the same problem in two views merges into one line listing both views', () => {
  const problem = { section: 'Flow', figure: 'Fig 1 "Flow chart"', what: '"A" overlaps "B"' };

  const lines = mergeProblems([
    { view: { width: 390, theme: 'light' }, problems: [problem] },
    { view: { width: 390, theme: 'dark' }, problems: [problem] },
  ]);

  assert.deepEqual(lines, ['✗ Flow · Fig 1 "Flow chart" · "A" overlaps "B" [views: 390 light, 390 dark]']);
});

test('a problem outside any figure prints its section and what only', () => {
  const lines = mergeProblems([
    { view: { width: 1440, theme: 'light' }, problems: [{ section: 'Wide', figure: null, what: 'page scrolls sideways' }] },
  ]);

  assert.deepEqual(lines, ['✗ Wide · page scrolls sideways [views: 1440 light]']);
});

test('a clean check reports the smallest SVG text and exits 0', () => {
  const report = reportLines({ lines: [], smallest: 13.5, sheet: '/out/plan.sheet.png', full: null });

  assert.deepEqual(report, { lines: ['✓ no problems · smallest SVG text 13.5px', 'sheet: /out/plan.sheet.png'], code: 0 });
});

test('a check with problems lists them, the sheet and the full dir, and exits 1', () => {
  const report = reportLines({ lines: ['✗ Wide · x [views: 390 light]'], smallest: null, sheet: '/s.png', full: { dir: '/full', tiled: [] } });

  assert.deepEqual(report, { lines: ['✗ Wide · x [views: 390 light]', 'sheet: /s.png', 'full: /full'], code: 1 });
});

test('the full line names every tiled view with its tile count and file range', () => {
  const tiled = [{ view: { width: 390, theme: 'light' }, count: 3 }, { view: { width: 390, theme: 'dark' }, count: 3 }];

  const report = reportLines({ lines: [], smallest: null, sheet: '/s.png', full: { dir: '/full', tiled } });

  assert.deepEqual(report.lines.slice(2), [
    'full: /full',
    'full: 390 light is 3 tiles, 390-light-1.png to 390-light-3.png',
    'full: 390 dark is 3 tiles, 390-dark-1.png to 390-dark-3.png',
  ]);
});

test('six first screens lay out as two rows of three within 1200 by 960', () => {
  const row = (theme) => [1440, 1024, 390].map((width) => ({ label: `${width} ${theme}`, width, height: 900 }));

  const { width, height, cells } = sheetLayout([row('light'), row('dark')]);

  assert.deepEqual(
    { width, height, cells: cells.map(({ label, x, y, width: w, height: h }) => [label, x, y, w, h]) },
    {
      width: 1199,
      height: 808,
      cells: [
        ['1440 light', 16, 16, 577, 360],
        ['1024 light', 605, 16, 410, 360],
        ['390 light', 1027, 16, 156, 360],
        ['1440 dark', 16, 410, 577, 360],
        ['1024 dark', 605, 410, 410, 360],
        ['390 dark', 1027, 410, 156, 360],
      ],
    },
  );
});

const MAC_CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const MAC_CHROMIUM = '/Applications/Chromium.app/Contents/MacOS/Chromium';
const nowhere = () => null;

test('CHROME_PATH wins even when it does not exist and a Chrome is installed', () => {
  const found = findChrome({ CHROME_PATH: '/custom/chrome' }, () => true, (name) => `/usr/bin/${name}`);

  assert.equal(found, '/custom/chrome');
});

test('Google Chrome in /Applications is preferred over Chromium and over PATH', () => {
  const found = findChrome({}, () => true, (name) => `/usr/bin/${name}`);

  assert.equal(found, MAC_CHROME);
});

test('Chromium in /Applications is used when Google Chrome is absent', () => {
  const found = findChrome({}, (file) => file === MAC_CHROMIUM, (name) => `/usr/bin/${name}`);

  assert.equal(found, MAC_CHROMIUM);
});

test('on PATH, google-chrome is preferred over the later names', () => {
  const found = findChrome({}, () => false, (name) => (name === 'chromium' || name === 'google-chrome' ? `/usr/bin/${name}` : null));

  assert.equal(found, '/usr/bin/google-chrome');
});

test('on PATH, chromium-browser is found when it is the only one', () => {
  const found = findChrome({}, () => false, (name) => (name === 'chromium-browser' ? '/usr/bin/chromium-browser' : null));

  assert.equal(found, '/usr/bin/chromium-browser');
});

test('with no CHROME_PATH, no app and nothing on PATH there is no browser', () => {
  const found = findChrome({}, () => false, nowhere);

  assert.equal(found, null);
});

test('a name on PATH resolves to the first directory that holds it', () => {
  const holds = new Set(['/b/chromium', '/c/chromium']);

  const resolved = onPathIn({ PATH: ['/a', '/b', '/c'].join(path.delimiter) }, (file) => holds.has(file))('chromium');

  assert.equal(resolved, '/b/chromium');
});

test('a name missing from every PATH directory resolves to null', () => {
  const resolved = onPathIn({ PATH: '/a' }, () => false)('chromium');

  assert.equal(resolved, null);
});

test('an unset PATH resolves every name to null', () => {
  const resolved = onPathIn({}, () => true)('chromium');

  assert.equal(resolved, null);
});

test('checking without a browser fails naming CHROME_PATH, the app paths and the PATH names searched', async () => {
  const run = checkPage(fixture('clean.html'), { sheet: sheetIn(), chrome: null });

  await assert.rejects(run, (e) => [
    'CHROME_PATH',
    MAC_CHROME,
    MAC_CHROMIUM,
    'google-chrome, google-chrome-stable, chromium, chromium-browser',
  ].every((part) => e.message.includes(part)));
});

test('a page within the limit is one tile', () => {
  assert.deepEqual(tileRanges(900, 16000), [{ y: 0, height: 900 }]);
});

test('a page exactly the limit tall is one tile', () => {
  assert.deepEqual(tileRanges(16000, 16000), [{ y: 0, height: 16000 }]);
});

test('a page one pixel over the limit is a full tile and a 1px tile', () => {
  assert.deepEqual(tileRanges(16001, 16000), [{ y: 0, height: 16000 }, { y: 16000, height: 1 }]);
});

test('a page of several limits tiles with no gap and a short last tile', () => {
  assert.deepEqual(tileRanges(40000, 16000), [
    { y: 0, height: 16000 },
    { y: 16000, height: 16000 },
    { y: 32000, height: 8000 },
  ]);
});

test('the sheet defaults to <page name>.sheet.png beside the page', () => {
  const { sheet } = parseCli(['/out/plan.html']);

  assert.equal(sheet, '/out/plan.sheet.png');
});

test('the CLI passes a clean page with exit 0', inChrome, () => {
  const run = spawnSync(process.execPath, [path.join(here, '..', 'check.mjs'), fixture('clean.html'), '--sheet', sheetIn()], {
    encoding: 'utf8',
    env: { ...process.env, CHROME_PATH: chrome },
  });

  assert.deepEqual({ code: run.status, first: run.stdout.split('\n')[0] }, { code: 0, first: '✓ no problems · smallest SVG text 16px' });
});

test('a 2000px wide block is reported as sideways scrolling', inChrome, async () => {
  const lines = await linesOf('overflow.html');

  assert.deepEqual(lines, [
    '✗ Wide · page scrolls sideways: div.slab is 2000px wide [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]',
  ]);
});

test('8px SVG text is reported as too small', inChrome, async () => {
  const lines = await linesOf('small-text.html');

  assert.deepEqual(lines, [
    '✗ Labels · Fig 1 "Small labels" · SVG text "tiny label" renders at 8px, under 12px [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]',
  ]);
});

test('a node label wider than its rect is reported as leaving its shape', inChrome, async () => {
  const lines = await linesOf('node-overflow.html');

  assert.deepEqual(lines, [
    '✗ Flow · Fig 1 "Flow chart" · "a much longer label" runs out of its node shape [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]',
  ]);
});

test('text cut off by an overflow:hidden box is reported as clipped', inChrome, async () => {
  const lines = await linesOf('clipped.html');

  assert.deepEqual(lines, [
    '✗ Box · div.cell cuts off its content ("text that does not fit in the cell") [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]',
  ]);
});

test('a heading pulled onto its paragraph is reported as overlapping it', inChrome, async () => {
  const lines = await linesOf('heading.html');

  assert.deepEqual(lines, [
    '✗ Tight · heading "Tight" overlaps the p below it [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]',
  ]);
});

test('a page that grows its mobile layout viewport is still reported as sideways scrolling at 390', inChrome, async () => {
  const lines = await linesOf('scroll-box.html');

  assert.deepEqual(lines, [
    '✗ Diagram · page scrolls sideways: div.item is 700px wide [views: 390 light, 390 dark]',
  ]);
});

test('sideways scrolling caused by an element a scroll box does not clip still names that element', inChrome, async () => {
  const lines = await linesOf('pinned.html');

  assert.deepEqual(lines, [
    '✗ Pinned · page scrolls sideways: div.pinned is 2000px wide [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]',
  ]);
});

test('--full writes a page taller than one capture as consecutive tiles that cover its height', inChrome, () => {
  const full = mkdtempSync(path.join(tmpdir(), 'revy-full-'));

  const run = spawnSync(process.execPath, [path.join(here, '..', 'check.mjs'), fixture('tall.html'), '--sheet', sheetIn(), '--full', full], {
    encoding: 'utf8',
    env: { ...process.env, CHROME_PATH: chrome },
  });

  assert.deepEqual(
    { code: run.status, fileCount: readdirSync(full).length, heights: [1, 2, 3, 4, 5].map((n) => pngHeight(path.join(full, `390-light-${n}.png`))) },
    { code: 0, fileCount: 30, heights: [8000, 8000, 8000, 8000, 8000] },
  );
});

test('an absolutely positioned label escaping a scroll box is named as the cause of sideways scrolling, not the wide code beside it', inChrome, async () => {
  const lines = await linesOf('escaped.html');

  assert.deepEqual(lines, [
    '✗ Escaped · page scrolls sideways: span.sr-only is 1px wide [views: 390 light, 390 dark]',
  ]);
});

test('a drawing centred in a scroll box too narrow for it is reported as cut off on the left', inChrome, async () => {
  const lines = await linesOf('centred.html');

  assert.deepEqual(lines, [
    '✗ Centred · Fig 1 "Wide flow" · div.frame cuts off the left of svg (it cannot be scrolled to) [views: 390 light, 390 dark]',
  ]);
});

test('a control with no accessible name is reported', inChrome, async () => {
  const lines = await linesOf('unnamed.html');
  assert.deepEqual(lines, ['✗ Intro · button has no accessible name; give it text or an aria-label [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]']);
});

test('a sequence actor whose name sits under its stick figure is not reported', inChrome, async () => {
  const lines = await linesOf('actor.html');
  assert.deepEqual(lines, []);
});

test('a link to an id the page does not have is reported', inChrome, async () => {
  const lines = await linesOf('anchor.html');
  assert.deepEqual(lines, ['✗ Intro · link "there" points at #nowhere, which is not on the page [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]']);
});

test('a prose table cell narrower than twelve characters is reported on wide views', inChrome, async () => {
  const lines = await linesOf('narrow-cell.html');

  assert.deepEqual(lines, [
    '✗ Plan · table cell "a sentence squeezed thin" is 40px wide, under the 12ch a prose cell needs [views: 1440 light, 1440 dark, 1024 light, 1024 dark]',
  ]);
});

test('a timeline connector drawn over its step marker is reported', inChrome, async () => {
  const lines = await linesOf('rail.html');

  assert.deepEqual(lines, [
    '✗ Plan · the timeline connector crosses the marker of "First" [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]',
  ]);
});

test('code outside a code block without its grey pill is reported', inChrome, async () => {
  const lines = await linesOf('bare-code.html');

  assert.deepEqual(lines, [
    '✗ Intro · code "bare()" has no grey pill behind it [views: 1440 light, 1440 dark, 1024 light, 1024 dark, 390 light, 390 dark]',
  ]);
});

test('check.mjs run through a symlinked skill directory still checks the page', inChrome, () => {
  const link = path.join(mkdtempSync(path.join(tmpdir(), 'revy-link-')), 'render');
  symlinkSync(path.join(here, '..'), link);

  const run = spawnSync(process.execPath, [path.join(link, 'check.mjs'), fixture('clean.html'), '--sheet', sheetIn()], { encoding: 'utf8', env: { ...process.env, CHROME_PATH: chrome } });

  assert.deepEqual({ code: run.status, last: run.stdout.trim().split('\n').at(-1) }, { code: 0, last: '✓ check passed: no problems' });
});

test('a clean check ends on a pass verdict', () => {
  assert.equal(verdictLine({ lines: [] }), '✓ check passed: no problems');
});

test('a check with problems ends on a failure verdict counting them', () => {
  assert.equal(verdictLine({ lines: ['✗ a', '✗ b'] }), '✗ check failed: 2 problems');
});
