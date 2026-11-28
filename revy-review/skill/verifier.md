# Verifier

Inputs: repo, base, head, the review directory, the capabilities inventory `capabilities.md`.

You verify the review findings for the change `<base>..<head>` in `<repo>`: read `problem.json` and every
`reviews/*.json` in the review directory, and decide on your own judgement, without asking. Look code up with the tools `capabilities.md` lists, as
`reviewer.md`'s "Tools" section says.

Every test you run lives in a worktree of head under `<review directory>/scratch/`, with `<h>` = the first 7
characters of head: `proofs` on `revy/<h>/proofs` for tests that may prove a finding
(`git -C <repo> worktree add -B revy/<h>/proofs <review directory>/scratch/proofs <head>`), and `verify`, detached,
for experiments that remove or stub code (`git -C <repo> worktree add --detach <review directory>/scratch/verify
<head>`). Create each before its first use and apply the `isolation` line of `capabilities.md` in it with
`<n>` = `verify`; tear down everything it started before replying.

1. **Merge** findings with the same root cause into one, keeping every site and the highest severity
   (blocking > non-blocking > question > suggestion). Gate items from `problem.json` merge like reviewer
   findings, in round 1 only (`state.json` `round`; later rounds account none):
   each criterion `not_met` (blocking) or `partly` (non-blocking) as `C<k>`, each scope-creep entry as `S<k>` (a
   question: is it meant to be in this change?). A criterion unmet because of a defect a reviewer found IS that
   defect: add `C<k>` to that finding's `merged_from`. Items on the same sites asking the same question become one;
   a gate item stands alone only when no finding matches it.
2. **Walk** each merged finding: open the code and step through its `breaks` scenario.
   - A claim that a test would still pass without some code (passes by coincidence, cannot fail) is settled by
     running it: remove or stub that code in `verify` and run the test; when it cannot run, trace the test's
     assertions step by step. A test that would fail makes the claim false: drop it.
   - Keep every finding the code does not contradict. Drop one only on a contradiction you quote from the code
     yourself, and record it; an investigator's refutation of a suspicion is a lead to check, never by itself a
     reason to drop a reviewer's finding.
   - When you refute only the stated reason, judge the proposed fix on its merits: keep the finding if the fix
     still prevents a real failure.
   - A disputed finding — one reviewer's finding against another's `fine` at the same site or contract, or one
     walking cannot settle — is settled by running. So is a `blocking` finding without a proof in team mode
     (`state.json` `mode`), as evidence for the author; self mode's fix loop reproduces it test-first anyway.
     Write a test in `proofs` that drives its `breaks` scenario at head. A failing test proves it: commit it there
     and give the finding that `proof`. A test that does not fail is discarded (`git checkout -- . && git clean
     -fd` in `proofs`) before you go on, so later cherry-picks apply cleanly. A passing one refutes it only when it drives exactly the `breaks` input and asserts the
     outcome `breaks` calls wrong: drop it then, `refuted_by` the code that produced the right outcome, quoted.
     Every other finding is walked only.
   - A finding you cannot settle either way, by walking or a run, stays, as a `question`.
3. **Proofs**: gather the proving commits of the kept findings on `revy/<h>/proofs`, when any kept finding has
   one: `git cherry-pick` there, per kept finding without a proof of yours, one `proof.commit` of the findings merged into it (on a conflict keep
   both sides' tests). Run each proof's `cmd` there; write the kept finding's `proof` with the cherry-picked sha and the output
   you saw. A proof that no longer fails there leaves its finding without `proof`. That branch stays local, unpushed;
   `revy-review sync` names it in report.md.
   Then delete every per-reviewer branch (`git -C <repo> branch --list 'revy/<h>/proofs-*'`): remove its worktree
   (`git -C <repo> worktree remove --force <review directory>/scratch/proofs-<n>`), then `git -C <repo> branch -D`
   it. When no kept finding has a proof, delete `revy/<h>/proofs` and its worktree too, so no proofs branch remains
   and report.md has no Proofs section.
4. **Account** for every reviewer finding and gate item exactly once: in one kept finding's `merged_from`, or in
   `dropped`.

Write each kept finding's `title`, `breaks` and `fix` standalone: a reader who sees only that finding learns which
code, which sites and which behaviour, named directly rather than through ids (`F<k>`, `C<k>`, `S<k>`, `<n>.<k>`,
reviewer numbers).

Write `verified.json` in the review directory (`site` = `"path:line"`, repo-relative, line at head; `refuted_by` is
the code that contradicts the finding):

```json
{"findings": [{"id": "F1", "title": "…", "severity": "blocking" | "non-blocking" | "question" | "suggestion",
               "sites": ["path:line", …], "quote": "the exact line at the first site, at head",
               "breaks": "…", "fix": "…", "rule": "rule id" | null,
               "proof": {…as in reviewer.md, "commit": "sha on revy/<h>/proofs"} | null, "merged_from": ["<n>.<k>", …]}],
 "dropped": [{"from": "<n>.<k>", "refuted_by": "path:line", "why": "what the code there does, quoting it"}]}
```

Ids `F1`, `F2`, … in order: blocking first, then non-blocking, questions, suggestions.

Done when every item is accounted for, every kept finding has been walked in code, every disputed one (and in team mode
every blocking one without a proof) has been run or stays a question because it cannot run here, nothing the isolation recipe started is still
running, every kept proof is on
`revy/<h>/proofs`, and `git -C <repo> branch --list 'revy/<h>/proofs*'` prints `revy/<h>/proofs` alone when a
kept finding has a proof, nothing otherwise. `revy-review check`
validates the file; you may be sent its errors to fix. Reply `done` when the file is written.
