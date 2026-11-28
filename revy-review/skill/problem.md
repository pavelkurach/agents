# Problem gate

You decide, before any deep review, whether the change `<base>..<head>` in `<repo>` addresses the problem it is
meant to solve. Decide on your own and read whatever code you need — at head through `git show <head>:<path>` or
a detached worktree under the review directory's `scratch/`; never check out, reset or detach anything in
`<repo>` itself.

Inputs: repo, base, head, the sources (ticket, PR/MR description, linked issue, commit messages, or the owner's
one-line goal), the review directory.

1. **Statement.** The problem in one or two sentences. `source`: `ticket`, `description`, `commits`, `owner`, or
   `inferred` when no source states it and you infer it from the commits.
2. **Criteria.** The acceptance criteria `C1`, `C2`, …. Quote each from its source (`quote`, `implied: false`);
   one no source states but the problem clearly requires gets `quote: null`, `implied: true`. Quoted criteria are
   core; implied ones are not. A criterion states the goal, never the author's mechanism: "a cancelled order is
   never charged", not "charge() checks the cancelled flag". For each population the goal covers — every caller or
   principal kind, variant, mode, and every path that produces or shows the affected data — write the invariant it
   must hold, implied when no source states it.
3. **Out of scope.** What the sources exclude or defer.
4. **Verdicts.** For each criterion, find the code that meets it and walk a concrete scenario through it: `met`,
   `partly`, `not_met`, or `unverifiable` (the evidence is outside the repository), with every `path:line` (at
   head) that supports the verdict. Met only in a test, only on the happy path, or only on the paths the diff
   changed, is `partly`.
5. **Scope creep.** Every changed hunk that serves no criterion, grouped by cause: one entry `S1`, `S2`, … per
   cause, with every site sharing it (`sites`) and why it is unrelated. Three edits with one cause are one entry.
6. **blocks**: true when any core criterion is `not_met`, or no criterion is `met`.

Write `<review dir>/problem.json` (each site `"path:line"`, repo-relative, line at head):

```json
{"statement": "…", "source": "ticket" | "description" | "commits" | "owner" | "inferred",
 "criteria": [{"id": "C1", "text": "…", "quote": "source text" | null, "implied": false,
               "verdict": "met" | "partly" | "not_met" | "unverifiable", "evidence": ["path:line"]}],
 "out_of_scope": ["…"],
 "scope_creep": [{"id": "S1", "sites": ["path:line"], "why": "…"}],
 "blocks": false}
```

Done when every criterion has a verdict backed by evidence and every changed hunk serves a criterion or sits in a
scope-creep entry. Reply `done` when the file is written.
