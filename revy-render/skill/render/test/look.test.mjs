import assert from 'node:assert/strict';
import { test } from 'node:test';
import { LookError, parseLook } from '../lib/look.mjs';
import { render } from './helpers.mjs';

const LIST = '# T\n\n- one\n- two\n- three\n- four\n';
const TABLE = '# T\n\n| Lane | State |\n|---|---|\n| a | affected |\n| b | affected |\n| c | n/a |\n';

test('a look tail that is not a whole number is rejected naming the field', () => {
  assert.throws(() => parseLook({ tail: 2.5 }), new LookError('look.tail must be a whole number ≥ 1, or null'));
});

test('a list longer than the tail marks the items past it and adds a disclosure counting them', async () => {
  const { html } = await render(LIST, { look: { tail: 2 } });
  assert.ok(html.includes('<li>two</li>\n<li class="tail-item">three</li>\n<li class="tail-item">four</li>\n</ul>\n<details class="tail"><summary><span class="tail-more">show 2 more</span><span class="tail-less">show fewer</span></summary></details>'));
});

test('a list no longer than the tail is left whole', async () => {
  const { html } = await render(LIST, { look: { tail: 4 } });
  assert.equal(html.includes('class="tail'), false);
});

test('a table with more rows than the tail marks the rows past it', async () => {
  const { html } = await render(TABLE, { look: { tail: 1 } });
  assert.equal((html.match(/<tr class="row-k-[a-z]+ tail-row">/g) ?? []).length, 2);
});

test('a table with more rows than the tail adds a disclosure counting them', async () => {
  const { html } = await render(TABLE, { look: { tail: 1 } });
  assert.ok(html.includes('</table></div>\n<details class="tail"><summary><span class="tail-more">show 2 more rows</span>'));
});

test('a folded section with a table shows its row count in the summary', async () => {
  const { html } = await render('# T\n\n## Impact\n\n| Lane | State |\n|---|---|\n| a | affected |\n| b | n/a |\n', { look: { fold: ['Impact'] } });
  assert.ok(html.includes('<summary><h2>Impact</h2><span class="digest">2 rows</span></summary>'));
});

test('a folded section of prose shows its first sentence in the summary', async () => {
  const { html } = await render('# T\n\n## Orientation\n\nThe service sits behind the API. It writes to the store.\n', { look: { fold: ['Orientation'] } });
  assert.ok(html.includes('<summary><h2>Orientation</h2><span class="digest">The service sits behind the API.</span></summary>'));
});

test('a term: definition list under a Glossary label renders as a definition list', async () => {
  const { html } = await render('# T\n\nGlossary:\n- attempt: one call to `s3::put_object`\n- breaker: a failure counter\n');
  assert.ok(html.includes('<dl class="glossary">\n<div><dt>attempt</dt><dd>one call to <code class="code">s3::<wbr>put_object</code></dd></div>\n<div><dt>breaker</dt><dd>a failure counter</dd></div>\n</dl>'));
});

test('a term: definition list under a Glossary heading renders as a definition list', async () => {
  const { html } = await render('# T\n\n### Glossary\n\n- probe: the one call let through\n');
  assert.ok(html.includes('<dl class="glossary">\n<div><dt>probe</dt><dd>the one call let through</dd></div>\n</dl>'));
});

test('a glossary list with an item that is not term: definition stays a list', async () => {
  const { html } = await render('# T\n\nGlossary:\n- probe: one call\n- see the design notes\n');
  assert.equal(html.includes('<dl class="glossary">'), false);
});
