// Imperative shell: repositories on disk and their git state.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { parseRemote } from './remote.mjs';

export class FatalError extends Error {}

function git(cwd, args) {
  try {
    return execFileSync('git', ['-C', cwd, ...args], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
  } catch {
    return undefined;
  }
}

/** `--repo <path>` or `--repo <alias>=<path>`; the first unaliased repo is primary. */
export function parseRepoArgs(args) {
  const parsed = args.map((arg) => {
    const m = /^([A-Za-z0-9_-]+)=(.+)$/.exec(arg);
    return m ? { alias: m[1], root: m[2] } : { alias: undefined, root: arg };
  });
  const firstUnaliased = parsed.findIndex((r) => r.alias === undefined);
  const repos = parsed.map((r, i) => ({
    alias: r.alias ?? path.basename(path.resolve(r.root)),
    root: path.resolve(r.root),
    primary: i === firstUnaliased,
  }));
  const dup = repos.find((r, i) => repos.findIndex((o) => o.alias === r.alias) !== i);
  if (dup) throw new FatalError(`duplicate --repo alias "${dup.alias}"; give one an explicit alias=path`);
  return repos;
}

export function openRepo({ alias, root, primary }) {
  if (!existsSync(root) || !statSync(root).isDirectory()) {
    throw new FatalError(`--repo ${root} is not a directory`);
  }
  const top = git(root, ['rev-parse', '--show-toplevel']);
  let files;
  return {
    alias,
    root,
    primary,
    git: top ? gitInfo(top) : undefined,
    exists: (rel) => {
      const abs = path.join(root, rel);
      return abs.startsWith(root + path.sep) && existsSync(abs) && statSync(abs).isFile();
    },
    read: (rel) => readFileSync(path.join(root, rel), 'utf8'),
    listFiles: () => (files ??= (git(root, ['ls-files']) ?? '').split('\n').filter(Boolean)),
  };
}

export function gitInfo(top) {
  const rawRemote = git(top, ['remote', 'get-url', 'origin']);
  const { ref, guessed } = pickRef(top);
  return {
    top,
    rawRemote,
    remote: rawRemote ? parseRemote(rawRemote) : undefined,
    ref,
    guessedRef: guessed,
    inHead: (relToTop) => git(top, ['cat-file', '-e', `HEAD:${relToTop}`]) !== undefined,
    hasCommit: (sha) => git(top, ['cat-file', '-e', `${sha}^{commit}`]) !== undefined,
    hasAt: (sha, relToTop) => git(top, ['cat-file', '-e', `${sha}:${relToTop}`]) !== undefined,
    // `git diff --quiet` exits 1 when the working tree differs, which `git` reports as undefined.
    differsAt: (sha, relToTop) => git(top, ['diff', '--quiet', sha, '--', relToTop]) === undefined,
    readAt: (sha, relToTop) => git(top, ['show', `${sha}:${relToTop}`]),
  };
}

// A sha is only linkable once some remote branch contains it; otherwise the
// default branch is the best address the forge knows, and `main` is a guess the caller reports.
/** @returns {{ ref: string, guessed: boolean }} */
function pickRef(top) {
  const sha = git(top, ['rev-parse', 'HEAD']);
  const pushed = sha && (git(top, ['branch', '-r', '--contains', 'HEAD']) ?? '') !== '';
  if (pushed) return { ref: sha, guessed: false };
  const head = git(top, ['symbolic-ref', 'refs/remotes/origin/HEAD'])?.replace(/^refs\/remotes\/origin\//, '');
  return head ? { ref: head, guessed: false } : { ref: 'main', guessed: true };
}
