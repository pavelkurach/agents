---
name: revy-review
description: Review a change for what it breaks — a PR/MR (number or URL), a commit or range `A..B`, or self-review of the current branch, where `fix` fixes the findings and re-reviews. Invoked as /revy-review [target] [fix].
---

# revy-review

You orchestrate; subagents read the code. Dispatch each subagent with your harness's mechanism: tell it to read
the brief named below (in this skill's directory) and follow it, and give it the listed inputs. Launch parallel
subagents together and wait for each result before using it. You alone talk to the owner; subagents run to the end.
Each subagent runs on your own model and effort unless its step names another. With `fix`, the owner is away from
the moment they invoke it: no step waits for them, and 4a's final summary is the one thing they see.

Run every `revy-review …` command from the review directory (each reads `state.json` there). `revy-review check`
validates every file in it and the accounting across them; re-run it until **clean**: fix the errors it reports, or
send them, each prefixed with its file, to the subagent that wrote that file.

Write all finding text — `breaks`, `fix`, report bodies, comments — **standalone**: a reader who sees only that
text learns which code, which sites and which behaviour; it names them directly instead of internal ids (`F<k>`,
`C<k>`, `S<k>`, reviewer numbers).

## Target and state
`/revy-review [target] [fix]`
- no target: current branch vs its merge-base with the default branch. Uncommitted work is reviewed as a snapshot
  commit that leaves the branch and index alone: `head` = `git stash create` (empty output: the tree is clean, use
  `HEAD`); it omits untracked files, so tell the owner which ones the review does not see (with `fix`: in the
  final summary)
- PR/MR number or URL: its head (fetch it locally) vs its merge-base with its target branch
- `<commit>`: `commit^..commit`
- `<A>..<B>`: only that range; impact is still traced across the whole repository

Mode: `self` for the owner's own change (no target, own commit or range, or `fix` given); `team` for someone
else's PR/MR. Ask if unclear.

Review directory: `~/.revy/reviews/<repo dir name>/<target, with / replaced by ->/`. Every scratch file, copy and build
goes under its `scratch/`; probes and experiments run in git worktrees there, on local `revy/<h>/proofs-*` branches
(`reviewer.md`, "Proofs branch"), so the reviewed branch changes only in the fix loop. The verifier merges the kept
proofs onto `revy/<h>/proofs` and deletes the rest. When the review is done, delete `scratch/` and run
`git -C <repo> worktree prune`; `revy/<h>/proofs` stays when a finding is proven. A review there of the same
base and head: resume from it (the owner may have curated report.md). Of a different base or head: ask before
overwriting; with `fix`, rename it to `<dir>.<first 7 characters of its head>` instead. Done when `state.json` holds
`{"repo": "<abs path>", "target": "<as given>", "base": "<sha>", "head": "<sha>", "mode": "self" | "team", "round": 1, "skill": "<this skill's directory>"}`;
every later step uses this base and head.

## 0. Capabilities
Steps 0 and 0b run concurrently: dispatch the problem gate first, then take this inventory while it runs.

The owner runs every review interactively and wants it run with full capabilities. Inventory what this session can
use, by purpose rather than product:
- code intelligence — definitions, callers, references, call hierarchy — for each language the change touches
- runtime execution — the repo builds and its tests run here (toolchain, dependencies, services); the proof tests
  need it
- structured search (syntax-aware, such as ast-grep) and text search (such as ripgrep)

Reading source and grep are the floor. Judge from the repo what fits it: a language server for each language it
uses, its own build and test commands.

For each gap that would materially help navigation or bug detection, name what it adds and the exact command that
installs or enables it.

Team mode also needs revy-explain for step 3: when revy-explain's `SKILL.md` is not where this harness installs
skills (e.g. `~/.claude/skills/revy-explain/SKILL.md`), ask the owner for its path; when `~/.revy-explain/config.toml` sets no `renderer`, ask which skill renders explanations
(offering what revy-explain's step 1 offers) and write `renderer = "<skill>"` there.

Ask every suggestion and question in one batch and wait for the owner's answer: install (run the commands they
approve, then inventory again) or proceed without. With none, proceed straight on, with at most a one-line
inventory note to the owner. With `fix`, proceed with what exists and carry the suggestions to the final summary.

Work out the isolation recipe too, because reviewers, specialists, investigators and the verifier run tests in
parallel and must never share what a run writes. From the repo's test config (env files, docker-compose, fixtures,
conftest, CI), list everything a test run writes — databases, containers, compose project names, ports, temp and
cache dirs, sockets, lock files — and give each a per-subagent instance keyed by `<n>`: database `<db>_revy_<n>`,
`COMPOSE_PROJECT_NAME=revy-<h>-<n>`, a port offset, `TMPDIR` inside the worktree. Read-only caches (dependency
downloads) stay shared. Build output stays in each subagent's worktree, seeded by a copy-on-write clone of the
checkout's build dir where the filesystem supports it (`cp -c` on APFS, `cp --reflink=auto` on Linux), so no
subagent starts a cold build.

Once the isolation line is known, dispatch one background subagent for the baseline: in a detached worktree of
head (`<review directory>/scratch/baseline`) under the recipe with n = `baseline`, it runs the existing tests of
the packages the change touches and writes `baseline.md` in the review directory — each command, its pass/fail
summary, the failing test names — then tears down what it started. One run serves every reviewer, so none reruns
a suite the baseline covers; whoever needs a suite result before `baseline.md` is written runs that suite itself.

Write the final inventory to `capabilities.md` in the review directory, one line per purpose:
`- <purpose>: <tool and how to call it>` or `- <purpose>: none — <fallback>`, then
`- isolation: <recipe, with <n> for the subagent's number>` or `- isolation: none needed — <why>`. Its path is an
input of every reviewer, specialist, investigator and verifier. Done when `capabilities.md` holds the final
inventory and the isolation line, the baseline subagent is dispatched, and every suggestion and question has the owner's answer (with `fix`: is
recorded for the final summary).

## 0b. Problem gate
Collect the sources: ticket, PR/MR description, linked issue, commit messages in the range. With none of them:
self mode without `fix` asks the owner for a one-line goal; otherwise the gate infers it from the commits.
Dispatch one subagent with `problem.md`; inputs: repo, base, head, the sources (or the owner's goal), the review
directory. Done when `revy-review check` is clean.

`blocks` true (the change misses the problem, or a core criterion is `not_met`) stops the review here: write
`verified.json` from the gate items alone (shaped as `verifier.md` shapes gate items: `C<k>`, and one question per
`S<k>`), run `revy-review check` and `revy-review sync`, and report only the gate findings (with `fix`, nothing
yet). Self mode: offer to fix them first — with `fix`, fix them without asking, as 4a steps 4–5 do; this gate-fix
pass is round 0, runs at most once and does not count against 4a's cap. Team mode: the report says so, and you ask
whether to run the deep review anyway. The review that follows is a fresh one in its own review directory, with
`round` = 1 and `capabilities.md` copied in; this directory stays as the gate record.
- After a gate fix: target `base..<new head>`, directory named after it. Rerun 0b at the new head and take the
  fix step's last full suite run there as its `baseline.md`. When 0b still blocks, stop: the gate findings go to
  the final summary.
- Team mode's deep review: `state.json` keeps `target` = the PR/MR and `mode` = team, so 4b still posts to it;
  the directory is `<this directory>.deep`. Head did not move, so copy `problem.json` and `baseline.md` too.

 `partly` and `unverifiable` let the review continue.

## 1. Reviewers
- Decide the review depth from the diff stat and hunks; it sets L, the number of lens reviewers:
  - `full` (L = 4) — the change alters a contract with consumers outside the diff (signature, behaviour, default,
    error, schema, wire/file/config format), or touches concurrency, persistence or migrations, security or authz,
    money, cross-boundary error handling or public API. Typical for round 1 of a non-trivial change; these
    categories decide round 1 only.
  - `focused` (L = 2) — logic changes whose consumers all sit in the diff or are few and local.
  - `light` (L = 1) — no runtime behaviour reaches a consumer: tests only, docs or comments, formatting, renames
    whose every reference is in the diff, or a fix round after only minor fixes (4a).

  Unsure between two levels: the deeper one; recall beats speed on every tie. A fix round takes the depth 4a step
  6 set, without deciding again. Record it in `state.json` as
  `"depth": {"level": "full" | "focused" | "light", "why": "<one line>"}`; report.md shows it under
  `## Review depth`, so a wrong call is visible. Done when `state.json` holds `depth`.
- `revy-review rules` validates the rules and specialists (this skill's and the owner's `~/.revy-review/`) and
  prints the rules index, then one `Specialist: <name> (dispatch: <mode>) —
  <file>` line per specialist with that specialist's own index indented below it (none when its file names no
  `rules:`). Fix errors with the owner, or report them and continue without the broken rule or specialist; with
  `fix`, continue without them and list the errors in the final summary.
- Decide each specialist. `always`: dispatch it. `when relevant`: read its file against the diff; skip it only when
  the change holds nothing its prompt covers, and when unsure, dispatch it. Record every decision in `state.json`
  as `"specialists": {"<name>": "dispatched" | {"skipped": "<one-line reason>"}, …}`; report.md lists the skips
  with their reasons. The dispatched specialists are reviewers L+1, L+2, … in listed order. Done when `state.json`
  names every listed specialist.
- Dispatch together, as separate subagents whatever the diff size — each starts from a clean context, which is what
  makes their verdicts independent:
  - L lens reviewer subagents with `reviewer.md`, n = 1..L. `full`: reviewer 1 takes lenses 1+2 and reviewer 2
    lenses 3+4; reviewers 3 and 4 are a second independent draw of lenses 1+2 and 3+4. `focused`: reviewers 1 and 2
    alone. `light`: reviewer 1 takes all four lenses. Lenses:
    1. contracts the change touches and every caller, consumer and producer
    2. failure — concurrency, interleavings, partial failure, cancellation, resources
    3. tests and edge inputs — the input matrix the code branches on, tests that pass by coincidence, boundaries
    4. design and simplicity — how sibling code and existing helpers do the same job, misuse, duplication, simpler
       data shapes
  - one specialist subagent per dispatched specialist with `reviewer.md`, at its n, given its file in place of a
    lens and, as its rules index, the lines indented below its line, indent removed (none when there are none).

  Inputs for each: repo, base, head, n, lens, the capabilities inventory, the path of `baseline.md`, the problem statement and criteria from
  `problem.json`, the rules index (the output of `revy-review rules` up to its first `Specialist:` line), the output path `reviews/<n>.json` in
  the review directory. A lens reviewer also gets your `focus` note when you have one — what the change is and
  where its risk lies, e.g. "adds only a test: does it assert the claimed behaviour, fail without the fix, stay
  isolated and deterministic?". `light` always has one; a fix round's names the findings the fix addresses.
- Done when every dispatched subagent has returned and `revy-review check` is clean.

## 1b. Investigate
When any review file holds a `suspect` entry, group the suspicions by the code or question they share — the same
suspicion raised by several reviewers is one group — and dispatch together one investigator subagent per
independent group (few suspicions: one investigator), each with `investigator.md`, on the strongest model your
harness offers at its highest reasoning effort; inputs: repo, base, head, the review directory, the capabilities
inventory, the path of `baseline.md`, its group's suspicion ids, n = a distinct number after the dispatched
specialists, and with `fix` that the owner is away. Done when
every investigator has returned and `revy-review check` is clean: together their `investigations/<n>.json`
resolve every suspicion exactly once.
No suspicions: skip this step.

## 2. Verify
When the reviews hold no findings and, in round 1, `problem.json` no gate items (no `not_met` or `partly`
criterion, no scope creep), there is nothing to verify: write `verified.json` as `{"findings": [], "dropped": []}` yourself and delete
every `revy/<h>/proofs-*` branch and its worktree as `verifier.md` step 3 does. Otherwise dispatch one verifier
subagent with `verifier.md`, always separate from the reviewers; inputs: repo, base, head, the review directory,
the capabilities inventory. Done when `revy-review check` is clean; once `verified.json` exists it also requires
`reviews/1..L`, every dispatched specialist's file and, when there are suspicions, investigations resolving each.

## 3. Report
`revy-review sync` renders `report.md` from `verified.json` (it refuses a review that does not check clean). Your own notes for the owner (what you checked, assumptions, anything that
is not a finding) go in a `## Notes` section of report.md, which is never posted.

Team mode: right after sync, dispatch one background subagent to explain the report. revy-explain is not
model-invocable, so it reads revy-explain's `SKILL.md` at the path step 0 settled and follows it in `review-report`
mode on `<review dir>/report.md`, with the repo and the reviewed revision = head. Pre-answer revy-explain's step-1
questions in its brief: scope = that report; renderer = the one in `~/.revy-explain/config.toml`; any other
question it would ask, it settles by its recommended default. It returns the
render path, the Answer line and the Gaps. Give the owner the summary without waiting for it and go on to 4b;
hand the owner its path, Answer line and Gaps when it lands.

Done when sync is clean and — except in the fix loop, whose reports the owner does not read — the owner has the
path and a short summary: counts by severity, the blocking titles, the criteria verdicts, and the
unresolved suspicions (report.md `## Suspicions (unconfirmed)`, each with the check that settles it), and the
proofs branch when report.md's `## Proofs` names one.

## 4a. Self mode
Without `fix`, step 3's summary is the delivery. With `fix`, the loop runs on its own — review, fix, re-review,
fix … — with no question to the owner. Every round still runs steps 2 and 3 — verifier,
`verified.json`, `revy-review sync`, `report.md` — because each fix step reads its findings and `post: no`
exclusions from that round's `report.md`; no round's report is summarised to the owner or explained. Per round:
1. Exclusions: findings the owner excluded ("fix all except 4") or marked `post: no` in report.md (run
   `revy-review sync` first if it was edited).
2. Settle the questions with subagents, one per question, dispatched together, on the strongest model at its
   highest reasoning effort. Each follows `investigator.md` steps 1–2 for its question, except that it runs in a
   detached worktree of head (`git -C <repo> worktree add --detach <review directory>/scratch/q<k> <head>`) with
   no branch and no commit, and removes that worktree before replying; inputs: repo, head, `capabilities.md`, the
   path of `baseline.md`, the question finding's text, isolation n = `q<k>`. It writes no other file and replies `confirmed` with the concrete input and wrong outcome, or a one-line answer. Confirmed → a
   finding at the severity its defect deserves; otherwise the one-line answer stands. Unresolved suspicions go, with their checks,
   to the final summary unfixed.
3. Decide (new = neither excluded nor answered in an earlier round). The loop fixes what the change introduced
   or exposed: a finding whose wrong outcome is identical at base and that no criterion of `problem.json` covers
   is listed, not fixed — fixing it grows the change and feeds the next round. Base is the review's original
   base, and "identical at base" means the same code path runs there unchanged: a defect in code the change added
   is the change's, even when it copies a pattern base already had. Choose each fix on its merits, never to steer
   the next round's depth.
   - This round was `light`, or holds no new blocking or non-blocking finding: fix its new non-blocking findings
     and apply its local, cheap questions and suggestions (suite green), commit them as step 5 says, list the
     rest, and stop. Only a new blocking finding in a `light` round goes on to another round.
   - Otherwise fix every new finding not excluded: blocking and non-blocking ones, and the local, cheap questions
     and suggestions; list the rest.
4. Test first, per behavioural defect: bring its proof test onto the branch — `git cherry-pick --no-commit` its
   commit from `revy/<h>/proofs`, or port it into the repo's test layout when it is a standalone script; a defect
   without proof gets a new test where the code can run here. Run it and confirm it fails. A defect whose test ran
   but could not be made to fail at head stays unfixed, listed as not reproduced; one whose code cannot run here is
   fixed by reading, listed as fixed, not reproducible here. Suggestions and design findings get no test. Then apply the deepest fix the evidence supports and rerun: each new test passes and the
   existing suite stays green.
5. When nothing was fixed this round (every defect not reproduced or excluded), stop: there is no range to
   re-review. Otherwise commit the round once, tests and fixes together, staging exactly the files you edited (`git add <paths>`), so
   build output and caches stay out; the owner's uncommitted work stays uncommitted. Pushing is the owner's.
6. Re-review only the fix range `<commit before the fixes>..<round commit>` as its own target, never the whole
   change again (steps 1–3, reusing this review's `capabilities.md`, and `problem.json` for its statement and
   criteria only — gate items are accounted in round 1 alone; `round` = this round + 1,
   and writing step 4's last full suite run at the round commit as its `baseline.md`);
   impact is still traced across the whole repository. Depth follows the severity of what this round fixed, not
   step 1's categories, which decide round 1 alone (every fix of persistence code would otherwise re-run `full`):
   first match wins — `full` when a fix changes a signature, schema or wire/file/config format that consumers
   found in the repository outside the fix range read; `focused` when any blocking finding was fixed; `light`
   otherwise. Its `focus` names the findings the fix addresses, never the ones left open, so the reviewers judge
   them afresh. Then start the next round at 4a step 1.

At most 5 rounds, a guard against a loop that does not converge: round 5's new findings are listed, not fixed,
and the loop stops with what is still open.

Done when the loop has stopped and the owner has the final summary: per round, its depth, what it fixed and its
commit; then what is still open — unresolved suspicions with their checks, for the owner to settle; defects not
reproduced, and those fixed but not reproducible here; the listed minor findings; step 0's capability suggestions;
rules or specialist errors skipped; untracked files the review did not see; and, when the guard stopped the loop,
the findings still unfixed.

## 4b. Team mode
1. The owner curates `report.md` (per finding `post: yes|no`, severity, title, body; `## My findings` takes
   `- site [severity] — text`) or tells you ("drop 4", "make 6 a question, softer", "add: …") and you edit it.
   Write comment text standalone, in the owner's tone from `voice.md`. Run `revy-review sync` until clean (it reads
   back only the posted blocks and own findings); a site outside the diff is a warning — tell the owner it would
   post as a general comment.
2. `revy-review comments` prints the post-ready comments.
3. Confirm the scope with the owner: repository, PR/MR, how many comments, posted as a draft/pending review.
4. Post them as a draft/pending review with whatever forge tool exists, only to the repository and PR/MR the owner
   confirmed.
5. Done when every comment is confirmed inline at its intended `path:line`, or the owner has the list of those that
   are not, and the owner has the explanation's path (or why it failed).
