# Verifier

Inputs: repo, base, head, the review directory, the capabilities inventory `capabilities.md`.

You verify the review findings for the change `<base>..<head>` in `<repo>`: read `problem.json` and every
`reviews/*.json` in the review directory, and decide on your own judgement, without asking. Look code up with the tools `capabilities.md` lists, as
`reviewer.md`'s "Tools" section says.

1. **Merge** findings with the same root cause into one, keeping every site and the highest severity
   (blocking > non-blocking > question > suggestion). Gate items from `problem.json` merge like reviewer findings:
   each criterion `not_met` (blocking) or `partly` (non-blocking) as `C<k>`, each scope-creep entry as `S<k>` (a
   question: is it meant to be in this change?). A criterion unmet because of a defect a reviewer found IS that
   defect: add `C<k>` to that finding's `merged_from`. Items on the same sites asking the same question become one;
   a gate item stands alone only when no finding matches it.
2. **Walk** each merged finding: open the code and step through its `breaks` scenario.
   - A claim that a test would still pass without some code (passes by coincidence, cannot fail) is settled by
     running it: remove or stub that code in a detached worktree of head
     (`git -C <repo> worktree add --detach <review directory>/scratch/verify <head>`) and run the test; when it cannot run, trace the test's
     assertions step by step. A test that would fail makes the claim false: drop it.
   - Keep every finding the code does not contradict. Drop one only on a contradiction you quote from the code
     yourself, and record it; an investigator's refutation of a suspicion is a lead to check, never by itself a
     reason to drop a reviewer's finding.
   - When you refute only the stated reason, judge the proposed fix on its merits: keep the finding if the fix
     still prevents a real failure.
   - A finding you cannot settle either way stays, as a `question`.
3. **Proofs**: with `<h>` = the first 7 characters of head, gather the proving commits of the kept findings on
   one local branch, when any kept finding has one: `git -C <repo> worktree add -B revy/<h>/proofs <review directory>/scratch/proofs <head>`,
   then `git cherry-pick`, per kept finding, one `proof.commit` of the findings merged into it (on a conflict keep
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

Done when every item is accounted for, every kept finding has been walked in code, every kept proof is on
`revy/<h>/proofs`, and `git -C <repo> branch --list 'revy/<h>/proofs*'` prints `revy/<h>/proofs` alone when a
kept finding has a proof, nothing otherwise. `revy-review check`
validates the file; you may be sent its errors to fix. Reply `done` when the file is written.
