---
name: revy-review
description: Review a change for what it breaks — a PR/MR (number or URL), a commit or range `A..B`, or self-review of the current branch, where `fix` fixes the findings and re-reviews. Invoked as /revy-review [target] [fix].
---

# revy-review

You orchestrate; subagents read the code. Dispatch each subagent with your harness's mechanism: tell it to read
the brief named below (in this skill's directory) and follow it, and give it the listed inputs. Launch parallel
subagents together and wait for each result before using it. You alone talk to the owner; subagents run to the end.
Each subagent runs on your own model and effort unless its step names another.

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
  `HEAD`); it omits untracked files, so tell the owner which ones the review does not see
- PR/MR number or URL: its head (fetch it locally) vs its merge-base with its target branch
- `<commit>`: `commit^..commit`
- `<A>..<B>`: only that range; impact is still traced across the whole repository

Mode: `self` for the owner's own change (no target, own commit or range, or `fix` given); `team` for someone
else's PR/MR. Ask if unclear.

Review directory: `~/.revy/reviews/<repo dir name>/<target, with / replaced by ->/`. Every scratch file, copy and build
goes under its `scratch/`; probes and experiments run in git worktrees there, on local `revy/<h>/proofs-*` branches
(`reviewer.md`, "Proofs branch"), so the reviewed branch changes only in the fix loop. When the review is done,
delete `scratch/` and run `git -C <repo> worktree prune`; the branches stay. A review there of the same
base and head: resume from it (the owner may have curated report.md). Of a different base or head: ask before
overwriting. Done when `state.json` holds
`{"repo": "<abs path>", "target": "<as given>", "base": "<sha>", "head": "<sha>", "mode": "self" | "team", "round": 1, "skill": "<this skill's directory>"}`;
every later step uses this base and head.

## 0. Problem gate
Collect the sources: ticket, PR/MR description, linked issue, commit messages in the range. With none of them:
self mode asks the owner for a one-line goal; team mode lets the gate infer it from the commits.
Dispatch one subagent with `problem.md`; inputs: repo, base, head, the sources (or the owner's goal), the review
directory. Done when `revy-review check` is clean.

`blocks` true (the change misses the problem, or a core criterion is `not_met`) stops the review here: write
`verified.json` from the gate items alone (shaped as `verifier.md` shapes gate items: `C<k>`, and one question per
`S<k>`), run `revy-review check` and `revy-review sync`, and report only the gate findings. Self mode:
offer to fix them first; once fixed, run the full review (steps 1–3) on `base..<new head>` before any fix-loop
re-review. Team mode: the report says so, and you ask whether to run the deep review anyway. `partly` and
`unverifiable` let the review continue.

## 1. Reviewers
- `revy-review rules` validates the rules and specialists (this skill's and the owner's `~/.revy-review/`) and
  prints the rules index for the languages the change touches, then one `Specialist: <name> (dispatch: <mode>) —
  <file>` line per specialist with that specialist's own index indented below it (none when its file names no
  `rules:`). Fix errors with the owner, or report them and continue without the broken rule or specialist.
- Decide each specialist. `always`: dispatch it. `when relevant`: read its file against the diff; skip it only when
  the change holds nothing its prompt covers, and when unsure, dispatch it. Record every decision in `state.json`
  as `"specialists": {"<name>": "dispatched" | {"skipped": "<one-line reason>"}, …}`; report.md lists the skips
  with their reasons. The dispatched specialists are reviewers 5, 6, … in listed order. Done when `state.json`
  names every listed specialist.
- Dispatch together, as separate subagents whatever the diff size — each starts from a clean context, which is what
  makes their verdicts independent:
  - 4 lens reviewer subagents with `reviewer.md`, n = 1..4: reviewer 1 takes lenses 1+2 and reviewer 2 lenses 3+4;
    reviewers 3 and 4 are a second independent draw of lenses 1+2 and 3+4. Lenses:
    1. contracts the change touches and every caller, consumer and producer
    2. failure — concurrency, interleavings, partial failure, cancellation, resources
    3. tests and edge inputs — the input matrix the code branches on, tests that pass by coincidence, boundaries
    4. design and simplicity — how sibling code and existing helpers do the same job, misuse, duplication, simpler
       data shapes
  - one specialist subagent per dispatched specialist with `reviewer.md`, at its n, given its file in place of a
    lens and, as its rules index, the lines indented below its line, indent removed (none when there are none).

  Inputs for each: repo, base, head, n, lens, the problem statement and criteria from `problem.json`, the rules
  index (the output of `revy-review rules` up to its first `Specialist:` line), the output path `reviews/<n>.json` in
  the review directory.
- Done when every dispatched subagent has returned and `revy-review check` is clean.

## 1b. Investigate
When any review file holds a `suspect` entry, dispatch one investigator subagent with `investigator.md`, on the
strongest model your harness offers at its highest reasoning effort; inputs: repo, base, head, the review
directory, n = the next number after the last dispatched specialist (5 with none). Done when `revy-review check` is clean.
No suspicions: skip this step.

## 2. Verify
Dispatch one verifier subagent with `verifier.md`, always separate from the reviewers; inputs: repo, base, head,
the review directory. Done when `revy-review check` is clean; once `verified.json` exists it also requires every
lens reviewer's and dispatched specialist's `reviews/<n>.json` and, when there are suspicions, `investigations.json`.

## 3. Report
`revy-review sync` renders `report.md` from `verified.json` (it refuses a review that does not check clean). Your own notes for the owner (what you checked, assumptions, anything that
is not a finding) go in a `## Notes` section of report.md, which is never posted. Done when sync is clean and the
owner has the path and a short summary: counts by severity, the blocking titles, the criteria verdicts, and the
unresolved suspicions (report.md `## Suspicions (unconfirmed)`, each with the check that settles it), and the
proofs branch when report.md's `## Proofs` names one.

## 4a. Self mode
Deliver the report. With `fix`, run rounds, at most 3:
1. Exclusions: findings the owner excluded ("fix all except 4") or marked `post: no` in report.md (run
   `revy-review sync` first if it was edited).
2. Settle questions first: investigate each; confirmed → it becomes a finding to fix; otherwise answer it in one line.
   Unresolved suspicions are the owner's call: list them with their checks and fix only those the owner confirms.
3. Test first, per remaining finding: bring its proof test onto the branch — `git cherry-pick --no-commit` its
   commit from `revy/<h>/proofs`, or port it into the repo's test layout when it is a standalone script; a finding
   without proof gets a new test where the code can run here. Run it and confirm it fails. Then apply the deepest
   fix the evidence supports and rerun: the test passes and the existing suite stays green.
4. Commit the round once, tests and fixes together, staging exactly the files you edited (`git add <paths>`), so build output and caches
   stay out; the owner's uncommitted work stays uncommitted. Pushing is the
   owner's.
5. Re-review the fix range `<commit before the fixes>..<round commit>` as its own target (steps 1–3, reusing this
   review's `problem.json`, `round` = this round + 1).
6. Stop when a round's verified findings hold nothing new (only findings excluded or answered before).

Done when the owner knows, per round, what it fixed, answered and left open, and its commit.

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
   are not.
