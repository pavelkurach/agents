// Diagram tools on the page — zoom, full screen, step-through, tracing — driven in headless Chrome.
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

/** A rendered page open in Chrome with motion reduced, so scrolling to a step is immediate. */
async function open(md, { js = true } = {}) {
  const { html } = await render(md);
  const file = path.join(mkdtempSync(path.join(tmpdir(), 'revy-tools-')), 'page.html');
  writeFileSync(file, html);
  const page = await browser.newPage();
  await page.setJavaScriptEnabled(js);
  await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }]);
  await page.setViewport({ width: 1280, height: 1000 });
  await page.goto(pathToFileURL(file).href, { waitUntil: 'load' });
  return page;
}
const mermaid = (body, info = '') => `# T\n\n\`\`\`mermaid${info}\n${body}\n\`\`\`\n`;
const FLOW = mermaid('flowchart TD\n  A[PUT] --> B[guard]\n  B -->|open| F[503 BreakerOpen]:::failure\n  B -->|closed| C[put_object]\n  C -->|ok| D[commit row]', ' The guard');
const SEQUENCE = mermaid('sequenceDiagram\n  W->>A: PUT blob\n  A->>S: put_object\n  S-->>A: 200 etag\n  A-->>W: 201 Created');
const STATE = mermaid('stateDiagram-v2\n  [*] --> Pending\n  Pending --> Sending: guard admits\n  Sending --> Done: 200');
const SVG_FIGURE = '# T\n\n```svg-figure Two boxes\n<svg viewBox="0 0 200 100" aria-label="two boxes"><rect x="10" y="10" width="80" height="40"/></svg>\n```\n';
const GLOSSARY_FLOW = `${mermaid('flowchart LR\n  A[probe] --> B[Store]')}\n## Glossary\n\n- probe: the one call let through\n`;

/** Does `act` `n` times, one after another. */
const repeat = async (n, act) => { for (let i = 0; i < n; i += 1) await act(); };
const tool = (label) => `.fig-controls button[aria-label="${label}"]`;
const readout = (page) => page.$eval('.zoom-tools output', (o) => o.textContent);
const shownWidth = (page) => page.$eval('.diagram-frame svg.is-wide, .diagram-frame svg:only-of-type', (s) => Math.round(s.getBoundingClientRect().width));
const current = (page) => page.$$eval('.diagram-frame svg.pele:not(.is-narrow) .is-current', (els) => els.map((e) => e.dataset.id));
const caption = (page) => page.$eval('.step-caption', (c) => c.textContent);

// --- zoom -----------------------------------------------------------------------------------------------------

test('a Mermaid diagram shows labelled zoom tools', inChrome, async () => {
  const page = await open(FLOW);
  assert.deepEqual(await page.$$eval('.fig-group.zoom-tools button', (bs) => bs.map((b) => b.getAttribute('aria-label') ?? b.textContent)), ['Zoom out', 'Zoom in', 'Fit to frame', 'Full screen']);
});

test('the zoom tools are visible without hovering the diagram', inChrome, async () => {
  const page = await open(FLOW);
  assert.equal(await page.$eval('.fig-controls', (c) => getComputedStyle(c).opacity), '1');
});

test('an SVG figure shows the zoom tools too', inChrome, async () => {
  const page = await open(SVG_FIGURE);
  assert.equal(await page.$$eval(`figure.figure-free ${tool('Zoom in')}`, (bs) => bs.length), 1);
});

test('zooming in redraws the diagram a quarter wider without a bitmap transform', inChrome, async () => {
  const page = await open(FLOW);
  const restWidth = await shownWidth(page);
  await page.click(tool('Zoom in'));
  assert.deepEqual([await shownWidth(page), await page.$eval('.diagram-frame svg', (s) => getComputedStyle(s).transform)], [Math.round(restWidth * 1.25), 'none']);
});

test('zooming in shows the new zoom level', inChrome, async () => {
  const page = await open(FLOW);
  await page.click(tool('Zoom in'));
  assert.equal(await readout(page), '125%');
});

test('Ctrl and the wheel zoom the diagram in', inChrome, async () => {
  const page = await open(FLOW);
  await page.$eval('.diagram-frame', (f) => f.dispatchEvent(new WheelEvent('wheel', { deltaY: -100, ctrlKey: true, clientX: 600, clientY: 300, bubbles: true, cancelable: true })));
  assert.equal(await readout(page), '110%');
});

test('a two-finger pinch apart zooms the diagram in', inChrome, async () => {
  const page = await open(FLOW);
  await page.$eval('.diagram-frame', (f) => {
    const touch = (type, pointerId, clientX) => f.dispatchEvent(new PointerEvent(type, { pointerId, pointerType: 'touch', clientX, clientY: 300, bubbles: true }));
    touch('pointerdown', 1, 500);
    touch('pointerdown', 2, 600);
    touch('pointermove', 2, 700);
  });
  assert.equal(await readout(page), '200%');
});

test('double-clicking a zoomed diagram resets it to its size at rest', inChrome, async () => {
  const page = await open(FLOW);
  await page.click(tool('Zoom in'));
  await page.$eval('.diagram-frame', (f) => f.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })));
  assert.equal(await readout(page), '100%');
});

test('dragging a zoomed diagram pans it', inChrome, async () => {
  const page = await open(FLOW);
  await repeat(5, () => page.click(tool('Zoom in')));
  const box = await page.$eval('.diagram-frame', (f) => { const r = f.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + 100 }; });
  await page.mouse.move(box.x, box.y);
  await page.mouse.down();
  await page.mouse.move(box.x - 40, box.y - 60, { steps: 4 });
  await page.mouse.up();
  assert.equal(await page.$eval('.diagram-frame', (f) => f.scrollTop > 0), true);
});

test('the plus key zooms in a diagram whose tools have focus', inChrome, async () => {
  const page = await open(FLOW);
  await page.focus(tool('Fit to frame'));
  await page.keyboard.press('+');
  assert.equal(await readout(page), '125%');
});

test('a node link still pops up its tip when zoomed', inChrome, async () => {
  const page = await open(GLOSSARY_FLOW);
  await page.click(tool('Zoom in'));
  await page.hover('a.node-link');
  assert.equal(await page.$eval('#revy-tip', (t) => (t.hidden ? null : t.textContent)), 'the one call let through');
});

test('without the script a diagram shows no tools and draws at its size', inChrome, async () => {
  const page = await open(FLOW, { js: false });
  assert.deepEqual([await page.$$eval('.fig-controls', (c) => c.length), await page.$eval('.diagram-frame svg', (s) => s.getBoundingClientRect().width > 0)], [0, true]);
});

// --- full screen ----------------------------------------------------------------------------------------------

test('full screen shows the figure in an overlay', inChrome, async () => {
  const page = await open(FLOW);
  await page.click(tool('Full screen'));
  assert.equal(await page.$$eval('.fig-overlay[role="dialog"] figure.diagram', (f) => f.length), 1);
});

test('full screen draws a small diagram larger to fill the screen', inChrome, async () => {
  const page = await open(FLOW);
  const restWidth = await shownWidth(page);
  await page.click(tool('Full screen'));
  assert.equal(await shownWidth(page) > restWidth, true);
});

test('Escape closes full screen and puts the figure back', inChrome, async () => {
  const page = await open(FLOW);
  await page.click(tool('Full screen'));
  await page.keyboard.press('Escape');
  assert.deepEqual([await page.$$eval('.fig-overlay', (o) => o.length), await page.$$eval('figure.diagram:not(.fig-overlay *)', (f) => f.length)], [0, 1]);
});

test('closing full screen returns focus to its button', inChrome, async () => {
  const page = await open(FLOW);
  await page.click(tool('Full screen'));
  await page.keyboard.press('Escape');
  assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Full screen');
});

// --- step-through ---------------------------------------------------------------------------------------------

test('a sequence step-through still advances and highlights after zooming', inChrome, async () => {
  const page = await open(SEQUENCE);
  await page.click(tool('Zoom in'));
  await page.click('[data-do="start"]');
  await page.click('[data-do="next"]');
  assert.deepEqual(await current(page), ['i1', 'i1']);
});

test('a flowchart step-through starts at the node no edge enters', inChrome, async () => {
  const page = await open(FLOW);
  await page.click('[data-do="start"]');
  assert.deepEqual(await current(page), ['A']);
});

test('the right arrow walks a flowchart along its main path, lighting the edge taken', inChrome, async () => {
  const page = await open(FLOW);
  await page.click('[data-do="start"]');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  assert.deepEqual(await current(page), ['L_B_C_0', 'L_B_C_0', 'C']);
});

test('a flowchart step shows the label of the edge taken as its caption', inChrome, async () => {
  const page = await open(FLOW);
  await page.click('[data-do="start"]');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  assert.equal(await caption(page), 'closed');
});

test('a flowchart step dims what is off its path', inChrome, async () => {
  const page = await open(FLOW);
  await page.click('[data-do="start"]');
  await page.keyboard.press('ArrowRight');
  assert.deepEqual(await page.$$eval('.diagram-frame svg.pele:not(.is-narrow) .pele-node.is-dim', (ns) => ns.map((n) => n.dataset.id)), ['F', 'C', 'D']);
});

test('the failure path is walked after the main path', inChrome, async () => {
  const page = await open(FLOW);
  await page.click('[data-do="start"]');
  await repeat(4, () => page.keyboard.press('ArrowRight'));
  assert.deepEqual(await current(page), ['L_B_F_0', 'L_B_F_0', 'F']);
});

test('a step along a failure edge marks its caption as a failure', inChrome, async () => {
  const page = await open(FLOW);
  await page.click('[data-do="start"]');
  await repeat(4, () => page.keyboard.press('ArrowRight'));
  assert.equal(await page.$eval('.step-caption', (c) => c.dataset.outcome), 'failure');
});

test('the left arrow steps back', inChrome, async () => {
  const page = await open(FLOW);
  await page.click('[data-do="start"]');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowLeft');
  assert.deepEqual(await current(page), ['A']);
});

test('Escape ends a step-through and shows the whole diagram', inChrome, async () => {
  const page = await open(FLOW);
  await page.click('[data-do="start"]');
  await page.keyboard.press('Escape');
  assert.deepEqual(await page.$$eval('.diagram-frame svg.pele .is-dim, .diagram-frame svg.pele .is-current', (els) => els.length), 0);
});

test('a state diagram steps transition by transition', inChrome, async () => {
  const page = await open(STATE);
  await page.click('[data-do="start"]');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  assert.deepEqual([await current(page), await caption(page)], [['edge1', 'edge1', 'Sending'], 'guard admits']);
});

test('a flowchart step-through works in full screen', inChrome, async () => {
  const page = await open(FLOW);
  await page.click(tool('Full screen'));
  await page.click('.fig-overlay [data-do="start"]');
  await page.keyboard.press('ArrowRight');
  assert.deepEqual(await current(page), ['L_A_B_0', 'B']);
});

test('a step outside a zoomed frame is scrolled into view', inChrome, async () => {
  const page = await open(FLOW);
  await repeat(5, () => page.click(tool('Zoom in')));
  await page.click('[data-do="start"]');
  await repeat(3, () => page.keyboard.press('ArrowRight'));
  assert.equal(await page.$eval('.diagram-frame', (f) => {
    const node = f.querySelector('svg.pele:not(.is-narrow) .pele-node.is-current').getBoundingClientRect();
    const box = f.getBoundingClientRect();
    return node.top >= box.top && node.bottom <= box.bottom && node.left >= box.left && node.right <= box.right;
  }), true);
});

// --- tracing --------------------------------------------------------------------------------------------------

test('hovering a state lights its transitions', inChrome, async () => {
  const page = await open(STATE);
  await page.hover('.diagram-frame svg.pele:not(.is-narrow) .pele-node[data-id="Sending"]');
  assert.deepEqual(await page.$$eval('.diagram-frame svg.pele:not(.is-narrow) .pele-edge.is-lit', (es) => es.map((e) => e.dataset.id)), ['edge1', 'edge2']);
});

test('hovering an edge label lights the edge and the two nodes it joins', inChrome, async () => {
  const page = await open(FLOW);
  await page.hover('.diagram-frame svg.pele:not(.is-narrow) .pele-edge-label[data-id="L_B_F_0"]');
  assert.deepEqual(await page.$$eval('.diagram-frame svg.pele:not(.is-narrow) .is-lit', (es) => es.map((e) => e.dataset.id).sort()), ['B', 'F', 'L_B_F_0', 'L_B_F_0']);
});

test('a state diagram start dot stays out of the tab order, having no words to announce', inChrome, async () => {
  const page = await open(STATE);
  assert.deepEqual(await page.$$eval('.diagram-frame svg.pele:not(.is-narrow) .pele-node[tabindex]', (ns) => ns.map((n) => n.dataset.id)), ['Pending', 'Sending', 'Done']);
});
