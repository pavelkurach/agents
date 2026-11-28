import assert from 'node:assert/strict';
import { test } from 'node:test';
import { blobUrl, parseRemote } from '../lib/remote.mjs';

test('github ssh remote builds a blob url', () => {
  const remote = parseRemote('git@github.com:acme/widgets.git');
  assert.equal(blobUrl(remote, 'abc123', 'src/a.rs', { start: 7 }), 'https://github.com/acme/widgets/blob/abc123/src/a.rs#L7');
});

test('github https remote builds a blob url', () => {
  const remote = parseRemote('https://github.com/acme/widgets.git');
  assert.equal(blobUrl(remote, 'main', 'src/a.rs'), 'https://github.com/acme/widgets/blob/main/src/a.rs');
});

test('self-hosted gitlab remote uses the /-/blob form', () => {
  const remote = parseRemote('git@gitlab.corp.example:team/sub/app.git');
  assert.equal(blobUrl(remote, 'main', 'lib/x.py', { start: 3 }), 'https://gitlab.corp.example/team/sub/app/-/blob/main/lib/x.py#L3');
});

test('bitbucket remote uses the /src form', () => {
  const remote = parseRemote('https://bitbucket.org/acme/widgets.git');
  assert.equal(blobUrl(remote, 'main', 'a.js'), 'https://bitbucket.org/acme/widgets/src/main/a.js');
});

test('unknown host is not a recognised forge', () => {
  assert.equal(parseRemote('git@example.org:acme/widgets.git'), undefined);
});
