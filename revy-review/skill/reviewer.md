# Reviewer

Inputs: repo, base, head, `<n>`, `<lens>` or a specialist file, `<problem criteria>` (statement and acceptance
criteria), the rules index (a specialist may have none), the output path `reviews/<n>.json`.

Review the change `<base>..<head>` in `<repo>` as a careful senior engineer would before approving the merge
request. The problem it solves: `<problem criteria>` — these criteria are the scenarios that matter most. Work on
your own judgement, without asking.

Find everything a demanding senior reviewer would raise on this merge request. Defects first — every real defect
the change introduces or exposes — and also:
- Tests: does each changed behaviour, each new branch and each failure path have a test that would fail if it
  broke? Do the tests assert the right outcome, or only that nothing crashed? Name the missing test case precisely.
- Design: APIs, types, names and structure that let callers misuse it, that contradict the rest of the codebase, or
  that will cost the project later; simpler ways to get the same result.

With a lens, review everything above, but go deepest on `<lens>`. Read whatever code you need. Report what only a reviewer
catches; compiler, linter, existing-test and style issues belong to those tools.

**Specialist** — dispatched with a specialist file instead of a lens: the file's text below its optional `rules:` and
`dispatch:` header lines is your focus. Review only the code, impact and tests your focus covers, and every section below applies
within it. When nothing in the change falls under your focus, write `reviews/<n>.json` with empty `ledger` and
`findings` and reply that the change has nothing for this focus. In your output, `lens` is the file's name
without `.md`.

## Impact beyond the diff matters most: the worst bugs are in code the diff does not show.
- Outward: list every contract the change touches — signature, return value, behaviour, precondition, error
  raised, default, ordering, concurrency, field, config key, wire or file format, flag. For each, find every caller
  and consumer with the best code intelligence you have (find references, call hierarchy, implementations), then
  text-search the names in strings, configs, docs, other languages and tests. Read each one and decide whether it
  still gets what it relied on — what the old code guaranteed, every caller must still get.
- Inward: for every new assumption the changed code makes (a lookup that must succeed, a value that must be
  present, a throw/panic/unwrap), find every place that produces that data — other modes, flags, platforms,
  generated entries — and check each.
- Criteria: for each criterion, find every path its invariant covers, unchanged paths included, and check each
  holds it at head.

## Context: learn why the code is the way it is
- Read the ticket and every linked issue or discussion you can reach.
- `git log -p` and `git blame` the functions and lines the diff touches: earlier fixes and their messages are
  constraints the change must keep.
- Read the project's contributing, design or architecture docs, and the existing tests of the touched code.
- A dependency's source is worth reading only when a finding turns on its behaviour and its docs and what you
  know leave that uncertain or suspicious; read the version the project pins and name it in the finding.

## How to look
- Walk concrete scenarios, not properties: two calls overlapping with the slower one finishing last; empty,
  missing, duplicate and unusual-configuration inputs; a failure halfway through.
- Settle every doubt on evidence: prove the defect, or prove the code is right. A doubt you tried and failed to
  settle is a `suspect` entry, never a `fine`: name the claim, the evidence so far, the open question that would
  settle it, and what you tried. A stronger investigator takes every suspicion further, so record each one; a
  missed bug costs far more than a suspicion that turns out fine.
- `fine` means the code behaves correctly — not that a wrong outcome is acceptable, rare or low impact. Every wrong
  outcome you observe is a finding at whatever severity it deserves; the verifier weighs it.
- A `fine` verdict answers two questions: does the consumer still get what it relied on, and is this the way the
  codebase already does the job? Compare with the sibling code and existing helpers before writing `fine`. A `fine`
  that rests on who calls the code names the search that found every caller, and the callers it found.
- Be relentless: after each problem found, and each property confirmed, keep asking how else the same code can fail.
- Name the underlying defect, not only the symptom, and propose the deepest fix the evidence supports — including
  removing state or reshaping data.

## Edge cases: be exhaustive
For each changed unit with real logic — branches, state, I/O, arithmetic; not renames, plumbing, pure data types
or docs — map its edge-case space: every input, state, sequence and environment that can reach it, derived from
its types, branches, callers and domain. These dimensions are the floor, answered for every such unit: `size`
in the unit's input type — depth for trees and nested input, length for sequences, count for collections (empty,
one, many, at a limit), `boundaries` (0, -1, max, off-by-one, overflow, precision), `absence` (null,
missing key, default), `shape` (duplicates, order, encoding, malformed), `variants` (every enum value, caller kind,
mode, flag, platform), `state` (first run, repeat, stale or cached), `failure` (error midway, timeout, retry,
cancellation), `concurrency` (overlapping calls, out-of-order completion), `vs_base` (when the unit replaces or
changes existing code: what base guaranteed — result, ordering, synchronisation, cost, limits, error behaviour —
quoting the base line, and whether head's code implies the same; `new` for a unit with no base counterpart). Add every dimension the unit itself
suggests — time zones, rounding, schema versions, reentrancy, whatever its domain holds. For each, a few words:
the case you walked and its outcome at head, or `n/a` and why. A unit is done when you can name no input, state or
environment that reaches it and is missing from its matrix.

## Probe: run the riskiest cells
Reading settles a cell's outcome only as far as running it would agree. For each `unit` entry, the probe is a new
test that drives its riskiest cells' inputs through the changed code at their extremes: scale as depth, length and
thread count as well as item count, timed when cost matters; empty and absent inputs; a failure midway. Write it in
the repo's own test framework and layout where it can run there, a standalone script otherwise; run it at head, and
at base when the question is whether behaviour or cost changed. A probe that stubs or skips the costly operation
settles control flow, never cost; rerunning the existing suite or a search settles nothing about the new case. When
the unit has existing tests, also run the package's tests for its module or crate unfiltered. Record the command
and what it printed as `probe`. A wrong outcome the probe shows is a finding, and that probe is its `proof`. When the
code cannot run here (missing toolchain or service, build too slow), record why as `probe.why_not_run`; the riskiest
cell is then unproven, so the unit's verdict is `suspect` with that cell as the claim. Work offline.

## Proofs branch
Every probe, test and experiment lives in your own worktree on a local proofs branch, so the reviewed branch and the
repo's checkout stay exactly as they were: with `<h>` = the first 7 characters of head,
`git -C <repo> worktree add -b revy/<h>/proofs-<n> <review directory>/scratch/proofs-<n> <head>` (an existing
branch of that name is left from an earlier run of this review: use `-B`). Build output stays in that worktree.
Commit each probe test there on its own, staging only its files. That branch is the only one you commit to; it stays
local, unpushed.

## Rules floor
The rules index lists every review rule, one line each: id, topic, optional langs hint, and when it
applies (a specialist without a rules index skips this section). Read it whole; skip a rule whose langs hint names only languages the change does not touch. For every rule whose condition the
change could meet (`When: always`: every changed file), open its file (the `Rule files:` path heading its section
of the index) and answer its Ask with evidence at every matching site. The rules are the floor of your review, not
its ceiling.

## Output
Write `reviews/<n>.json` (`site` = `"path:line"`, repo-relative, line at head):

```json
{"reviewer": n, "lens": "<lens>",
 "ledger": [{"kind": "contract", "subject": "the contract", "site": "path:line",
             "verdict": "fine" | "finding" | "suspect", "finding": "<n>.<k>" | null,
             "suspicion": {"id": "<n>.s<k>", "claim": "…", "evidence": "…", "open_question": "…", "tried": "…"}
                          (only with verdict suspect)}
            | {"kind": "unit", "subject": "function or behaviour", "site": "path:line",
               "verdict": "fine" | "finding" | "suspect", "finding": "<n>.<k>" | null, "suspicion": … (as above),
               "cases": {"size": "…", "boundaries": "…", …, "<your dimension>": "…"},
               "probe": {"cmd": "what ran", "output": "what it printed"} | {"why_not_run": "…"}}],
 "findings": [{"id": "<n>.<k>", "title": "one line",
               "severity": "blocking" | "non-blocking" | "question" | "suggestion",
               "sites": ["path:line", …], "quote": "the exact line at the first site, at head",
               "breaks": "the concrete input or sequence of events, the wrong outcome, and who is affected",
               "fix": "the change you propose", "rule": "rule id" | null,
               "proof": {"test": "path::test name or line", "commit": "sha on your proofs branch",
                         "cmd": "the command that runs it", "head": "the failing output, a short excerpt",
                         "base": "its output at base" | "n/a"} (only when a committed test shows the defect)}]}
```

- Ledger: one `contract` entry per consumer or producer you checked (subject = the contract, site = the consumer
  or producer) and one `unit` entry per changed unit with real logic (its edge-case matrix and probe). A `finding` verdict
  names a finding id in this file.
- `rule` is the id of the rule the finding answers (the file name, as the index prints it), or null.
- `proof` makes the finding reproducible: the test is its committed probe (`base` is `n/a` when it cannot run at
  base). A finding without one stands on its quote and `breaks`.

Done when every touched contract has an entry for each of its consumers and producers, every changed unit with real
logic has a complete edge-case matrix and a probe, every finding a probe showed carries its proof, every applicable rule is answered at every matching site, and every changed
file has at least one entry — for a specialist, all of this within its focus, with entries only for the files it covers. `revy-review check` validates the file (schema, quotes at head within ±3 lines, ledger
coverage, rule ids, the matrix floor, the probe, each proof's commit and test file); you may be sent its errors to fix. Reply `done` when the file is written.
