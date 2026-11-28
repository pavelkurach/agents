import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { renderPlan } from '../lib/plan.mjs';
import { openRepo } from '../lib/repos.mjs';

const git = (cwd, ...args) => execFileSync('git', ['-C', cwd, ...args], { stdio: 'pipe' });

/** A committed temp repo; `files` maps path -> content. */
export function tempRepo({ files = {}, remote } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'revy-render-'));
  git(root, 'init', '-q', '-b', 'trunk');
  git(root, 'config', 'user.email', 't@t');
  git(root, 'config', 'user.name', 't');
  Object.entries(files).forEach(([p, c]) => {
    mkdirSync(path.dirname(path.join(root, p)), { recursive: true });
    writeFileSync(path.join(root, p), c);
  });
  if (remote) git(root, 'remote', 'add', 'origin', remote);
  git(root, 'add', '-A');
  git(root, 'commit', '-q', '-m', 'init');
  return root;
}

export const pushed = (root) => git(root, 'update-ref', 'refs/remotes/origin/trunk', 'HEAD');
export const headSha = (root) => git(root, 'rev-parse', 'HEAD').toString().trim();

export const render = (planText, { roots = [], look, link, evidence } = {}) =>
  renderPlan({
    planText,
    planName: 'plan.md',
    lookJson: look,
    repos: roots.map((root, i) => openRepo({ alias: `r${i}`, root, primary: i === 0 })),
    link,
    evidence,
  });

/** Commit `files` (path -> content) on top of HEAD; returns the new sha. */
export function commit(root, files) {
  Object.entries(files).forEach(([p, c]) => {
    mkdirSync(path.dirname(path.join(root, p)), { recursive: true });
    writeFileSync(path.join(root, p), c);
  });
  git(root, 'add', '-A');
  git(root, 'commit', '-q', '-m', 'next');
  return headSha(root);
}

/** Overwrite files in the working tree without committing. */
export function edit(root, files) {
  Object.entries(files).forEach(([p, c]) => writeFileSync(path.join(root, p), c));
}

/** Visible text of the HTML, tags removed and entities decoded. */
export const textOf = (html) =>
  html
    .replace(/<style[\s\S]*?<\/style>/g, '')
    .replace(/<[^>]+>/g, '')
    .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&');
