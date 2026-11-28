#!/usr/bin/env node
import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { LookError } from './lib/look.mjs';
import { renderPlan } from './lib/plan.mjs';
import { FatalError, openRepo, parseRepoArgs } from './lib/repos.mjs';
import { LinkBaseError, parseLinkBase } from './lib/remote.mjs';
import { EvidenceError, parseEvidence } from './lib/evidence.mjs';
import { copyDifference, sourceLineOf } from './lib/verbatim.mjs';

const USAGE = 'usage: node render.mjs --plan <plan.md> (--out <dir> | --lint) [--source <original.md>] [--look <look.json>] [--repo <path>|<alias>=<path>]... [--link <web-url>@<sha>] [--evidence <log.md>]';

function read(file, what) {
  try {
    return readFileSync(file, 'utf8');
  } catch (e) {
    throw new FatalError(`cannot read ${what} ${file}: ${e.code ?? e.message}`);
  }
}

/** The copy must be its source plus inserted figures; anything else is a change to the content. */
function checkCopy(copyFile, copyText, sourceFile) {
  const diff = copyDifference(copyText, read(sourceFile, 'source'));
  if (!diff) return;
  throw new FatalError(`${copyFile}:${diff.copyLine} differs from ${sourceFile}:${diff.sourceLine} outside the inserted figures:\n`
    + `  copy:   ${diff.copyText ?? '(ends here)'}\n  source: ${diff.sourceText ?? '(ends here)'}`);
}

/** Write via a sibling temp file and rename, so a reader never sees a half-written page. */
function writeAtomic(target, text) {
  const temp = `${target}.${process.pid}.tmp`;
  writeFileSync(temp, text);
  renameSync(temp, target);
}

async function main(argv) {
  let values;
  try {
    ({ values } = parseArgs({
      args: argv,
      options: {
        plan: { type: 'string' }, out: { type: 'string' }, look: { type: 'string' }, source: { type: 'string' },
        repo: { type: 'string', multiple: true }, link: { type: 'string' }, evidence: { type: 'string' }, lint: { type: 'boolean' },
      },
    }));
  } catch (e) {
    throw new FatalError(`${e.message}\n${USAGE}`);
  }
  if (!values.plan || !(values.out || values.lint)) throw new FatalError(`--plan and --out are required\n${USAGE}`);
  const planText = read(values.plan, 'plan');
  if (values.source) checkCopy(values.plan, planText, values.source);
  let lookJson;
  if (values.look) {
    try {
      lookJson = JSON.parse(read(values.look, 'look file'));
    } catch (e) {
      if (e instanceof FatalError) throw e;
      throw new FatalError(`look file ${values.look} is not valid JSON: ${e.message}`);
    }
  }
  const repos = parseRepoArgs(values.repo ?? []).map(openRepo);
  const link = values.link === undefined ? undefined : parseLinkBase(values.link);
  const primary = repos.find((r) => r.primary);
  if (link && primary?.git && !primary.git.hasCommit(link.ref)) {
    throw new FatalError(`--link sha ${link.ref} is not a commit in --repo ${primary.root}; fetch it or pick a sha the repo has`);
  }
  // The page, its header and its warnings name the document the reader knows: the source, not the render copy.
  const docName = path.basename(values.source ?? values.plan);
  const copyName = path.basename(values.plan);
  const toSource = values.source ? sourceLineOf(planText, read(values.source, 'source')) : (n) => n;
  const locate = (n) => {
    const s = toSource(n);
    return s === undefined ? `${copyName}:${n}` : `${docName}:${s}`;
  };
  const ledger = values.evidence === undefined ? undefined
    : { name: path.basename(values.evidence), ...parseEvidence(read(values.evidence, 'evidence'), path.basename(values.evidence)) };
  const { html, warnings, errors } = await renderPlan({ planText, planName: docName, locate, lookJson, repos, link, evidence: ledger });
  if (values.lint) {
    // Lint: everything is parsed and drawn as for a render, then nothing is written.
    warnings.forEach((w) => console.log(`${errors.includes(w) ? 'error' : 'warn'}: ${w}`));
    return errors.length ? 1 : 0;
  }
  const target = path.join(values.out, `${path.parse(docName).name}.html`);
  mkdirSync(values.out, { recursive: true });
  writeAtomic(target, html);
  warnings.forEach((w) => console.log(`warn: ${w}`));
  console.log(`wrote ${path.resolve(target)}`);
  return 0;
}

main(process.argv.slice(2)).then((code) => { process.exitCode = code; }, (e) => {
  if (!(e instanceof FatalError || e instanceof LookError || e instanceof LinkBaseError || e instanceof EvidenceError)) throw e;
  console.error(`error: ${e.message}`);
  process.exit(1);
});
