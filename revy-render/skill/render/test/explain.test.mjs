import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseLook } from '../lib/look.mjs';
import { render } from './helpers.mjs';

const FAQ = '# T\n\n## Map\n\nx\n\n## FAQ\n\n### Why two?\n\nBecause.\n\n### And three?\n\nNo.\n';

test('a Trust: line becomes the trust callout', async () => {
  const { html } = await render('# T\n\n**Trust:** 9 claims · 9 verified\n\n## A\n\nx\n');
  assert.ok(html.includes('<p class="trust"><strong>Trust:</strong> 9 claims · 9 verified</p>'));
});

test('an Answer: line becomes the answer callout', async () => {
  const { html } = await render('# T\n\nAnswer: it filters by language.\n');
  assert.ok(html.includes('<p class="answer">Answer: it filters by language.</p>'));
});

test('an Edges: line under a figure is set as a figure note', async () => {
  const { html } = await render('# T\n\n## A\n\nEdges: A→B E3\n');
  assert.ok(html.includes('<p class="edges">Edges: A→B E3</p>'));
});

test('look collapse is a list of section titles', () => {
  assert.deepEqual(parseLook({ collapse: ['FAQ'] }).collapse, ['FAQ']);
});

test('look collapse that is not a list of strings is rejected', () => {
  assert.throws(() => parseLook({ collapse: 'FAQ' }), { message: 'look.collapse must be an array of strings' });
});

test('a collapsed section turns each ### entry into a closed disclosure', async () => {
  const { html } = await render(FAQ, { look: { collapse: ['FAQ'] } });
  assert.ok(html.includes('<details class="entry" id="why-two"><summary><h3>Why two?</h3></summary>\n<p>Because.</p>\n</details>'));
});

test('a collapsed section lists its entries in the side navigation', async () => {
  const { html } = await render(FAQ, { look: { collapse: ['FAQ'] } });
  assert.ok(html.includes('<li><a href="#faq">FAQ</a><ol class="toc-sub"><li><a href="#why-two">Why two?</a></li><li><a href="#and-three">And three?</a></li></ol></li>'));
});

test('a collapse naming no section warns', async () => {
  const { warnings } = await render(FAQ, { look: { collapse: ['Questions'] } });
  assert.deepEqual(warnings, ['look.collapse names unknown section "Questions" — look file']);
});
