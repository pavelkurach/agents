#!/usr/bin/env node
// Opens a rendered page in the system Chrome at three widths in light and dark, and reports what a reader would see as broken:
// sideways scrolling, SVG text under 12px, labels that leave their shape or overlap, content cut off by its box, headings
// that sit on what follows, controls without an accessible name, and in-page links to ids the page lacks. Writes a contact sheet of every view's first screen so an agent can look without scripting a browser;
// with --full it also writes each view's whole page, in tiles when the page is taller than one Chrome capture allows.
// Exit codes: 0 clean, 1 problems found, 2 no browser or any other error.
import { existsSync, mkdirSync, realpathSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import puppeteer from 'puppeteer-core';
import { escapeHtml } from './lib/escape.mjs';

const USAGE = [
  'usage: node check.mjs <page.html> [--sheet <out.png>] [--full <dir>]',
  '  --sheet  contact sheet of every view\'s first screen (default <page name>.sheet.png beside the page)',
  '  --full   whole page per view as <width>-<theme>.png, or <width>-<theme>-<n>.png tiles when taller than one capture',
  '  exit 0 clean, 1 problems found, 2 no browser or error (set CHROME_PATH to choose the browser)',
].join('\n');
const MAC_CHROMES = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
];
const PATH_CHROMES = ['google-chrome', 'google-chrome-stable', 'chromium', 'chromium-browser'];
const NO_CHROME = [
  'no Chrome or Chromium found. Searched: the CHROME_PATH environment variable (unset);',
  `${MAC_CHROMES.join(', ')};`,
  `then PATH for ${PATH_CHROMES.join(', ')}.`,
  'Install Chrome or set CHROME_PATH to the browser executable.',
].join(' ');
const MIN_TEXT_PX = 12;
const MIN_HEADING_GAP_PX = 6;
const VIEW_HEIGHT = 900;
// Chrome cannot capture more than 16384px in one shot
// Chrome repeats content in very tall captures; 8000 px tiles stay well inside what it draws faithfully.
const TILE_MAX_HEIGHT = 8000;
const FONT_WAIT_MS = 5000;
const LOAD_TIMEOUT_MS = 15000;
// ≈1.15 megapixels: the most a model takes in before it scales an image down
const SHEET = { maxWidth: 1200, maxHeight: 960, pad: 16, gap: 12, label: 22 };

export const WIDTHS = [1440, 1024, 390];
export const THEMES = ['light', 'dark'];
export const VIEWS = WIDTHS.flatMap((width) => THEMES.map((theme) => ({ width, theme })));
export const viewLabel = ({ width, theme }) => `${width} ${theme}`;

export class UsageError extends Error {}

/** argv -> { page, sheet, full } with absolute paths; the sheet defaults to `<page dir>/<page name>.sheet.png`. */
export function parseCli(argv) {
  let parsed;
  try {
    parsed = parseArgs({
      args: argv,
      allowPositionals: true,
      options: { sheet: { type: 'string' }, full: { type: 'string' } },
    });
  } catch (e) {
    throw new UsageError(`${e.message}\n${USAGE}`);
  }
  const { values, positionals } = parsed;
  if (positionals.length !== 1) throw new UsageError(`expected exactly one page, got ${positionals.length}\n${USAGE}`);
  const page = path.resolve(positionals[0]);
  const { dir, name } = path.parse(page);
  return {
    page,
    sheet: path.resolve(values.sheet ?? path.join(dir, `${name}.sheet.png`)),
    full: values.full === undefined ? null : path.resolve(values.full),
  };
}

/**
 * The Chrome to launch, or null when there is none: CHROME_PATH, then the macOS apps, then PATH names in order.
 * An explicit CHROME_PATH is trusted, so a wrong one fails at launch.
 * @param {{ CHROME_PATH?: string }} env
 * @param {(file: string) => boolean} exists
 * @param {(name: string) => string | null} onPath
 * @returns {string | null}
 */
export function findChrome(env, exists, onPath) {
  if (env.CHROME_PATH) return env.CHROME_PATH;
  return MAC_CHROMES.find((file) => exists(file)) ?? PATH_CHROMES.map((name) => onPath(name)).find((file) => file !== null) ?? null;
}

/**
 * A resolver for command names against env.PATH: the first directory holding the name wins; null when none does.
 * @param {{ PATH?: string }} env
 * @param {(file: string) => boolean} exists
 * @returns {(name: string) => string | null}
 */
export const onPathIn = (env, exists) => (name) =>
  (env.PATH ?? '').split(path.delimiter).filter(Boolean).map((dir) => path.join(dir, name)).find((file) => exists(file)) ?? null;

/**
 * The consecutive vertical ranges that cover a page of `total` px with no gap, each at most `limit` px.
 * @param {number} total
 * @param {number} limit
 * @returns {{ y: number, height: number }[]}
 */
export const tileRanges = (total, limit) =>
  Array.from({ length: Math.ceil(total / limit) }, (_, i) => ({ y: i * limit, height: Math.min(limit, total - i * limit) }));

const tileFile = ({ width, theme }, n) => `${width}-${theme}-${n}.png`;

const problemKey = ({ section, figure, what }) => JSON.stringify([section, figure, what]);

/** [{ view, problems: [{ section, figure, what }] }] -> one `✗` line per distinct problem, listing the views it shows in. */
export function mergeProblems(viewResults) {
  const merged = viewResults.reduce((acc, { view, problems }) => {
    problems.forEach((p) => {
      const key = problemKey(p);
      const seen = acc.get(key) ?? { problem: p, views: [] };
      const label = viewLabel(view);
      acc.set(key, seen.views.includes(label) ? seen : { ...seen, views: [...seen.views, label] });
    });
    return acc;
  }, new Map());
  return [...merged.values()].map(({ problem: { section, figure, what }, views }) =>
    `✗ ${[section, figure, what].filter((part) => part !== null).join(' · ')} [views: ${views.join(', ')}]`);
}

/** The smallest SVG text size seen in any view, or null when no view has SVG text. */
export const smallestText = (viewResults) =>
  viewResults.reduce((min, { smallest }) => (smallest === null || (min !== null && min <= smallest) ? min : smallest), null);

/**
 * Everything the CLI prints after a finished check, and its exit code. `full` is null, or the output dir plus the views
 * that were written as `count` tiles.
 * @param {{ lines: string[], smallest: number | null, sheet: string, full: { dir: string, tiled: { view: { width: number, theme: string }, count: number }[] } | null }} check
 * @returns {{ lines: string[], code: 0 | 1 }}
 */
/** The last line a check prints, so a run that printed nothing can never pass for a clean one. */
export const verdictLine = ({ lines }) => (lines.length
  ? `✗ check failed: ${lines.length} problem${lines.length === 1 ? '' : 's'}`
  : '✓ check passed: no problems');

export function reportLines({ lines, smallest, sheet, full }) {
  const summary = lines.length
    ? lines
    : [`✓ no problems · ${smallest === null ? 'no SVG text' : `smallest SVG text ${smallest}px`}`];
  return {
    lines: [
      ...summary,
      `sheet: ${sheet}`,
      ...(full === null ? [] : [
        `full: ${full.dir}`,
        ...full.tiled.map(({ view, count }) => `full: ${viewLabel(view)} is ${count} tiles, ${tileFile(view, 1)} to ${tileFile(view, count)}`),
      ]),
    ],
    code: lines.length ? 1 : 0,
  };
}

/**
 * Rows of { label, width, height } -> a sheet at most SHEET.maxWidth × SHEET.maxHeight. One scale for every cell keeps
 * the widths comparable; each cell carries its own fields through, so the caller can attach its image.
 */
export function sheetLayout(rows, { maxWidth, maxHeight, pad, gap, label } = SHEET) {
  const rowHeight = (row) => Math.max(...row.map((c) => c.height));
  const fitWidth = (row) => (maxWidth - 2 * pad - gap * (row.length - 1)) / row.reduce((sum, c) => sum + c.width, 0);
  const fitHeight = (maxHeight - 2 * pad - gap * (rows.length - 1) - label * rows.length) / rows.reduce((sum, row) => sum + rowHeight(row), 0);
  const scale = Math.min(1, fitHeight, ...rows.map(fitWidth));
  const placed = rows.reduce(
    ({ cells, y }, row) => {
      const h = Math.floor(rowHeight(row) * scale);
      const { cells: rowCells } = row.reduce(
        (acc, c) => {
          const w = Math.floor(c.width * scale);
          return { cells: [...acc.cells, { ...c, x: acc.x, y, width: w, height: Math.floor(c.height * scale) }], x: acc.x + w + gap };
        },
        { cells: [], x: pad },
      );
      return { cells: [...cells, ...rowCells], y: y + label + h + gap };
    },
    { cells: [], y: pad },
  );
  const right = Math.max(...placed.cells.map((c) => c.x + c.width));
  return { width: right + pad, height: placed.y - gap + pad, label, cells: placed.cells };
}

/** The sheet as a page; each cell needs `png` (base64). */
export const sheetHtml = ({ width, height, label, cells }) => `<!doctype html>
<body style="margin:0;background:#fff">
<div id="sheet" style="position:relative;width:${width}px;height:${height}px;background:#fff;font:600 13px/18px system-ui,sans-serif;color:#333">
${cells.map((c) => `<div style="position:absolute;left:${c.x}px;top:${c.y}px;width:${c.width}px">
<div style="height:${label - 4}px;margin-bottom:4px;white-space:nowrap">${escapeHtml(c.label)}</div>
<img src="data:image/png;base64,${c.png}" width="${c.width}" height="${c.height}" style="display:block;outline:1px solid #ccc">
</div>`).join('\n')}
</div>
</body>`;

// ---- in the page: these functions are serialised by page.evaluate, so they use only browser globals and their own body ----

function waitForFonts(ms) {
  return Promise.race([document.fonts.ready, new Promise((resolve) => setTimeout(resolve, ms))]);
}

function openAllDetails() {
  document.querySelectorAll('details:not([open])').forEach((d) => { d.open = true; });
  return new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
}

/** Every check in one pass. -> { smallest: px | null, problems: [{ section, figure, what }] } */
function inspectPage({ minText, headingGap, viewWidth }) {
  const SVG_HIDDEN = 'defs, marker, symbol, clipPath, mask, pattern';
  const CLIPS = new Set(['hidden', 'clip']);
  const REPLACED = new Set(['IMG', 'VIDEO', 'CANVAS', 'IFRAME', 'INPUT', 'TEXTAREA', 'SELECT', 'OBJECT', 'EMBED']);
  const root = document.documentElement;
  const body = document.body;
  const clean = (s) => s.replace(/\s+/g, ' ').trim();
  const cut = (s, n) => (s.length > n ? `${s.slice(0, n)}…` : s);
  const quote = (s) => `"${cut(clean(s), 40)}"`;
  const visible = (el) => el.checkVisibility({ opacityProperty: true, visibilityProperty: true });
  const describe = (el) => `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ''}${[...el.classList].slice(0, 2).map((c) => `.${c}`).join('')}`;
  const ancestors = (el) => {
    const out = [];
    for (let a = el.parentElement; a && a !== body && a !== root; a = a.parentElement) out.push(a);
    return out;
  };
  const figures = [...document.querySelectorAll('main figure')];
  const locate = (el) => {
    const sec = el.closest('section[id], details[id]');
    const h2 = sec?.querySelector('h2');
    const fig = el.closest('figure');
    const n = fig ? figures.indexOf(fig) : -1;
    const caption = fig ? clean(fig.querySelector('figcaption')?.textContent || fig.querySelector('svg[aria-label]')?.getAttribute('aria-label') || '') : '';
    return {
      section: sec ? (h2 && clean(h2.textContent)) || sec.id : 'page',
      figure: fig ? `${n >= 0 ? `Fig ${n + 1}` : 'figure'}${caption ? ` "${cut(caption, 40)}"` : ''}` : null,
    };
  };
  const at = (el, what) => ({ ...locate(el), what });
  const htmlElements = [...body.querySelectorAll('*')].filter((el) => el instanceof HTMLElement);

  const pageOverflow = () => {
    // Mobile emulation widens the layout viewport to fit the content, so innerWidth would hide the overflow: compare with the requested width.
    if (root.scrollWidth <= viewWidth) return [];
    // an element inside a box that clips or scrolls sideways cannot widen the page, unless it is absolutely positioned
    // out of that box; a fixed element never widens it
    const contained = (el) => ancestors(el).some((a) => getComputedStyle(a).overflowX !== 'visible');
    const pastEdge = [...body.querySelectorAll('*')]
      .filter((el) => !el.parentElement.closest('svg') && visible(el) && getComputedStyle(el).position !== 'fixed')
      .filter((el) => el.getBoundingClientRect().right > viewWidth + 1 && (getComputedStyle(el).position === 'absolute' || !contained(el)));
    const widest = (els) => els
      .filter((el) => !els.some((o) => o !== el && o.contains(el)))
      .map((el) => ({ el, width: Math.round(el.getBoundingClientRect().width) }))
      .sort((a, b) => b.width - a.width)
      .slice(0, 3);
    const culprits = widest(pastEdge);
    return culprits.length
      ? culprits.map(({ el, width }) => at(el, `page scrolls sideways: ${describe(el)} is ${width}px wide`))
      : [at(body, `page scrolls sideways: ${describe(body)} is ${root.scrollWidth}px wide`)];
  };

  const svgTexts = [...document.querySelectorAll('svg text')]
    .filter((t) => clean(t.textContent) && !t.closest(SVG_HIDDEN) && visible(t))
    .map((t) => ({ t, r: t.getBoundingClientRect(), ctm: t.getScreenCTM() }))
    .filter(({ r, ctm }) => ctm && r.width > 0 && r.height > 0)
    .map((x) => ({ ...x, px: Math.round(parseFloat(getComputedStyle(x.t).fontSize) * Math.hypot(x.ctm.a, x.ctm.b) * 10) / 10 }));
  const rootSvg = (el) => {
    let svg = el.closest('svg');
    while (svg.parentElement?.closest('svg')) svg = svg.parentElement.closest('svg');
    return svg;
  };
  const bySvg = svgTexts.reduce((acc, x) => acc.set(rootSvg(x.t), [...(acc.get(rootSvg(x.t)) ?? []), x]), new Map());

  const smallText = () => [...bySvg].flatMap(([, texts]) => {
    const small = texts.filter((x) => x.px < minText).sort((a, b) => a.px - b.px);
    if (!small.length) return [];
    const more = small.length > 1 ? ` (+${small.length - 1} more)` : '';
    return [at(small[0].t, `SVG text ${quote(small[0].t.textContent)} renders at ${small[0].px}px, under ${minText}px${more}`)];
  });

  const inset = (r, dx, dy) => ({ left: r.left + dx, right: r.right - dx, top: r.top + dy, bottom: r.bottom - dy });
  const inside = (a, b) => a.left >= b.left && a.right <= b.right && a.top >= b.top && a.bottom <= b.bottom;
  const union = (rs) => ({
    left: Math.min(...rs.map((r) => r.left)), right: Math.max(...rs.map((r) => r.right)),
    top: Math.min(...rs.map((r) => r.top)), bottom: Math.max(...rs.map((r) => r.bottom)),
  });
  const area = (r) => Math.max(0, r.right - r.left) * Math.max(0, r.bottom - r.top);
  const textFit = () => [...bySvg].flatMap(([svg, texts]) => {
    const box = svg.getBoundingClientRect();
    // a drawing with overflow: visible paints past its box on purpose
    const svgClips = getComputedStyle(svg).overflow !== 'visible';
    const escapes = texts.flatMap(({ t, r }) => {
      // a sequence actor's name is drawn under its stick figure on purpose, so only the drawing's edge bounds it
      const node = t.closest('g.pele-node:not(.pele-actor-actor)');
      const shapes = node
        ? [...node.querySelectorAll('rect, polygon, path, circle, ellipse')].filter((s) => s.closest('g.pele-node') === node && !s.closest(`text, ${SVG_HIDDEN}`))
        : [];
      const ink = inset(r, 1, 1);
      if (shapes.length) return inside(ink, union(shapes.map((s) => s.getBoundingClientRect()))) ? [] : [at(t, `${quote(t.textContent)} runs out of its node shape`)];
      return !svgClips || inside(ink, box) ? [] : [at(t, `${quote(t.textContent)} runs past the edge of its drawing`)];
    });
    // a text's box is its em box; the letters sit in about its middle 56%, so stacked labels may touch boxes without touching letters
    const inks = texts.map(({ t, r }) => ({ t, b: inset(r, 1, r.height * 0.22) }));
    const overlaps = inks.flatMap((a, i) => inks.slice(i + 1).flatMap((b) => {
      const shared = area({ left: Math.max(a.b.left, b.b.left), right: Math.min(a.b.right, b.b.right), top: Math.max(a.b.top, b.b.top), bottom: Math.min(a.b.bottom, b.b.bottom) });
      return shared > 0.1 * Math.min(area(a.b), area(b.b)) ? [at(a.t, `${quote(a.t.textContent)} overlaps ${quote(b.t.textContent)}`)] : [];
    }));
    return [...escapes, ...overlaps];
  });

  const clipped = () => {
    // ellipsis and line clamps cut text on purpose; a 1-2px box is a visually-hidden label
    const intended = (el, cs) => cs.textOverflow === 'ellipsis' || cs.webkitLineClamp !== 'none' || el.clientWidth <= 2 || el.clientHeight <= 2;
    const candidates = htmlElements.filter((el) => !REPLACED.has(el.tagName) && visible(el));
    const overflowing = candidates.filter((el) => {
      const cs = getComputedStyle(el);
      return !intended(el, cs) && (
        (CLIPS.has(cs.overflowX) && el.scrollWidth > el.clientWidth + 1) || (CLIPS.has(cs.overflowY) && el.scrollHeight > el.clientHeight + 1));
    });
    const reported = new Set(overflowing);
    const ownText = (el) => [...el.childNodes].some((n) => n.nodeType === Node.TEXT_NODE && n.textContent.trim());
    // a text pushed out to the left or top of a clipping box does not grow its scroll size, so it is found from the text
    const escaped = candidates.filter(ownText).flatMap((el) => {
      const clip = ancestors(el).find((a) => {
        const cs = getComputedStyle(a);
        return cs.overflowX !== 'visible' || cs.overflowY !== 'visible';
      });
      if (!clip || reported.has(clip)) return [];
      const cs = getComputedStyle(clip);
      if (intended(clip, cs)) return [];
      const r = el.getBoundingClientRect();
      const c = clip.getBoundingClientRect();
      const left = c.left + clip.clientLeft;
      const top = c.top + clip.clientTop;
      const outX = CLIPS.has(cs.overflowX) && (r.left < left - 1 || r.right > left + clip.clientWidth + 1);
      const outY = CLIPS.has(cs.overflowY) && (r.top < top - 1 || r.bottom > top + clip.clientHeight + 1);
      if (!outX && !outY) return [];
      reported.add(clip);
      return [{ clip, text: el.textContent }];
    });
    // a drawing centred in a box narrower than itself overflows to the left too, where no scrolling reaches it
    const leftCut = [...document.querySelectorAll('svg')]
      .filter((svg) => !svg.parentElement.closest('svg') && visible(svg))
      .flatMap((svg) => {
        const clip = ancestors(svg).find((a) => getComputedStyle(a).overflowX !== 'visible');
        if (!clip || reported.has(clip)) return [];
        const left = clip.getBoundingClientRect().left + clip.clientLeft - clip.scrollLeft;
        if (svg.getBoundingClientRect().left >= left - 1) return [];
        reported.add(clip);
        return [at(clip, `${describe(clip)} cuts off the left of svg (it cannot be scrolled to)`)];
      });
    return [
      ...leftCut,
      ...overflowing.map((el) => at(el, `${describe(el)} cuts off its content (${quote(el.textContent)})`)),
      ...escaped.map(({ clip, text }) => at(clip, `${describe(clip)} cuts off its content (${quote(text)})`)),
    ];
  };

  const headingCollisions = () => {
    const laidOut = (el) => {
      if (!visible(el)) return false;
      const cs = getComputedStyle(el);
      return cs.position !== 'absolute' && cs.position !== 'fixed' && el.getBoundingClientRect().height > 0;
    };
    const block = (el) => { const d = getComputedStyle(el).display; return !d.startsWith('inline') && d !== 'contents'; };
    const following = (h) => {
      for (let s = h.nextElementSibling; s; s = s.nextElementSibling) if (laidOut(s)) return s;
      const walker = document.createTreeWalker(body, NodeFilter.SHOW_ELEMENT);
      walker.currentNode = h;
      for (let n = walker.nextNode(); n; n = walker.nextNode()) if (!h.contains(n) && laidOut(n) && block(n)) return n;
      return null;
    };
    return [...document.querySelectorAll('main h1, main h2, main h3, main h4')].filter(laidOut).flatMap((h) => {
      const next = following(h);
      if (!next) return [];
      const hr = h.getBoundingClientRect();
      const nr = next.getBoundingClientRect();
      // content beside the heading (a badge in the same flex row) is not content below it
      if (nr.left >= hr.right || nr.right <= hr.left) return [];
      const gap = nr.top - hr.bottom;
      if (gap >= headingGap) return [];
      const what = gap < 0
        ? `heading ${quote(h.textContent)} overlaps the ${next.tagName.toLowerCase()} below it`
        : `heading ${quote(h.textContent)} sits ${Math.round(gap)}px above the ${next.tagName.toLowerCase()} below it, under ${headingGap}px`;
      return [at(h, what)];
    });
  };

  // A prose cell squeezed under a readable width on a wide view: short-value columns (data-short) may be narrow.
  const narrowCells = () => (viewWidth < 1024 ? [] : [...document.querySelectorAll('.table-wrap > table td')]
    .filter((td) => !td.closest('figure, .ep') && clean(td.textContent) && visible(td))
    .filter((td) => !(td.closest('.table-wrap').dataset.short ?? '').split(' ').includes(String(td.cellIndex + 1)))
    .flatMap((td) => {
      const style = getComputedStyle(td);
      const probe = document.createElement('span');
      probe.textContent = '0';
      td.append(probe);
      const ch = probe.getBoundingClientRect().width;
      probe.remove();
      const width = td.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      return width < 12 * ch - 0.5 ? [at(td, `table cell ${quote(td.textContent)} is ${Math.round(width)}px wide, under the 12ch a prose cell needs`)] : [];
    }));

  // A timeline's connector must end at a step marker or pass behind it: whatever paints at the marker's middle and
  // lower body (down to its bottom tip) is the marker itself.
  const railsOverMarkers = () => [...document.querySelectorAll('.tl-step .tl-dot')].filter(visible).flatMap((dot) => {
    dot.scrollIntoView({ block: 'center', behavior: 'instant' });
    const r = dot.getBoundingClientRect();
    const covered = [0.5, 0.75, 0.9].some((f) => {
      const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height * f);
      return hit && !dot.contains(hit);
    });
    const title = dot.closest('.tl-step').querySelector('.tl-title')?.textContent ?? '';
    return covered ? [at(dot, `the timeline connector crosses the marker of ${quote(title)}`)] : [];
  });

  // Code reads apart from prose only on its grey pill; code blocks carry their own frame.
  const barePills = () => [...document.querySelectorAll('code')]
    .filter((c) => !c.closest('pre') && visible(c) && clean(c.textContent))
    .filter((c) => ['transparent', 'rgba(0, 0, 0, 0)'].includes(getComputedStyle(c).backgroundColor))
    .map((c) => at(c, `code ${quote(c.textContent)} has no grey pill behind it`));

  // A control a keyboard or screen-reader user reaches must say what it is.
  const unnamed = () => [...document.querySelectorAll('a[href], button, [role="button"], [tabindex]:not([tabindex="-1"])')]
    .filter((el) => !el.closest(SVG_HIDDEN) && (el instanceof SVGElement || visible(el)))
    .filter((el) => !(el.getAttribute('aria-label') || el.getAttribute('aria-labelledby') || el.getAttribute('title')
      || clean(el.textContent) || el.querySelector('img[alt]:not([alt=""]), svg title')))
    .map((el) => at(el, `${describe(el)} has no accessible name; give it text or an aria-label`));

  const brokenAnchors = () => [...document.querySelectorAll('a[href^="#"]')]
    .filter((a) => a.getAttribute('href').length > 1 && !document.getElementById(decodeURIComponent(a.getAttribute('href').slice(1))))
    .map((a) => at(a, `link ${quote(a.textContent || a.getAttribute('aria-label') || '')} points at ${a.getAttribute('href')}, which is not on the page`));

  return {
    smallest: svgTexts.length ? Math.min(...svgTexts.map((x) => x.px)) : null,
    problems: [...pageOverflow(), ...smallText(), ...textFit(), ...clipped(), ...headingCollisions(), ...unnamed(), ...brokenAnchors(), ...narrowCells(), ...railsOverMarkers(), ...barePills()],
  };
}

// ---- shell ----

async function checkView(browser, url, { width, theme }, full) {
  const page = await browser.newPage();
  try {
    const mobile = width < 600;
    await page.setViewport({ width, height: VIEW_HEIGHT, deviceScaleFactor: 1, isMobile: mobile, hasTouch: mobile });
    await page.emulateMediaFeatures([{ name: 'prefers-color-scheme', value: theme }]);
    await page.goto(url, { waitUntil: 'load', timeout: LOAD_TIMEOUT_MS });
    await page.evaluate(waitForFonts, FONT_WAIT_MS);
    const firstScreen = await page.screenshot({ encoding: 'base64' });
    await page.evaluate(openAllDetails);
    let tileCount = 0;
    const { smallest, problems } = await page.evaluate(inspectPage, { minText: MIN_TEXT_PX, headingGap: MIN_HEADING_GAP_PX, viewWidth: width });
    if (full !== null) {
      // The checks may scroll; a clip is taken from the current scroll position, so return to the top first.
      await page.evaluate(() => window.scrollTo(0, 0));
      const tiles = tileRanges(await page.evaluate(() => document.documentElement.scrollHeight), TILE_MAX_HEIGHT);
      for (const [i, { y, height }] of tiles.entries()) {
        const png = await page.screenshot({ clip: { x: 0, y, width, height }, captureBeyondViewport: true });
        writeFileSync(path.join(full, tiles.length === 1 ? `${width}-${theme}.png` : tileFile({ width, theme }, i + 1)), png);
      }
      tileCount = tiles.length;
    }
    return { view: { width, theme }, smallest, problems, firstScreen, tileCount };
  } finally {
    await page.close();
  }
}

async function drawSheet(browser, results, file) {
  const rows = THEMES.map((theme) => WIDTHS.map((width) => ({
    label: viewLabel({ width, theme }),
    width,
    height: VIEW_HEIGHT,
    png: results.find((r) => r.view.width === width && r.view.theme === theme).firstScreen,
  })));
  const layout = sheetLayout(rows);
  const page = await browser.newPage();
  try {
    await page.setViewport({ width: layout.width, height: layout.height, deviceScaleFactor: 1 });
    await page.setContent(sheetHtml(layout), { waitUntil: 'load' });
    const sheet = await page.$('#sheet');
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, await sheet.screenshot());
  } finally {
    await page.close();
  }
}

/**
 * Checks one page in every view. -> { lines, smallest, sheet, full } shaped for `reportLines`. Rejects when there is no
 * Chrome (`chrome: null`) or the browser fails.
 */
export async function checkPage(file, { sheet, full = null, chrome = findChrome(process.env, existsSync, onPathIn(process.env, existsSync)) }) {
  if (chrome === null) throw new Error(NO_CHROME);
  const browser = await puppeteer.launch({ executablePath: chrome, headless: true, args: ['--hide-scrollbars', '--force-color-profile=srgb'] });
  try {
    if (full !== null) mkdirSync(full, { recursive: true });
    const url = pathToFileURL(file).href;
    const results = [];
    for (const view of VIEWS) results.push(await checkView(browser, url, view, full));
    await drawSheet(browser, results, sheet);
    const tiled = results.filter((r) => r.tileCount > 1).map((r) => ({ view: r.view, count: r.tileCount }));
    return { lines: mergeProblems(results), smallest: smallestText(results), sheet, full: full === null ? null : { dir: full, tiled } };
  } finally {
    if (browser.connected) await browser.close();
  }
}

async function main(argv) {
  const { page, sheet, full } = parseCli(argv);
  if (!existsSync(page)) throw new UsageError(`no such page: ${page}`);
  const result = await checkPage(page, { sheet, full });
  const { lines, code } = reportLines(result);
  lines.forEach((line) => console.log(line));
  console.log(verdictLine(result));
  return code;
}

// Compared as real paths: the skill is usually run through a symlink (~/.claude/skills/…), whose argv path differs
// from the module's own.
const isEntry = process.argv[1] && realpathSync(path.resolve(process.argv[1])) === realpathSync(fileURLToPath(import.meta.url));
if (isEntry) {
  main(process.argv.slice(2)).then(
    (code) => { process.exitCode = code; },
    (e) => {
      console.error(`error: ${e.message}`);
      process.exitCode = 2;
    },
  );
}
