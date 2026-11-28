# pull-request

The context a reader needs to review and understand the PR themselves: what changed, why, and what it reaches.

Fetch the PR with the host's CLI (`gh pr view <n> --json title,body,baseRefName,headRefName,files,commits`,
`gh pr diff <n>`; `glab mr view` / `glab mr diff` on GitLab), and its linked issue. Check out the head and the
base per evidence.md § Worktrees, so symbols resolve on both sides.

## Enumerate

| Enumerate | Scope |
|---|---|
| Purpose | the PR description, the linked issue, commit messages |
| Changed files | every file, grouped by purpose |
| Contracts | signatures, types, schemas, APIs, config, migrations whose shape changed — base → head |
| Flows | every flow whose behaviour differs — its entry points, actors, stages, and branches per how-it-works.md § Enumerate, at base and at head |
| Reach | consumers one hop out from every changed symbol, at head, and whether the PR touches each |
| Tests | added, changed, removed — what each asserts |
| Hunks | every hunk of the diff: what it does, why, and the flow or contract it belongs to |

## Body

The overview first, then the walk through the code.

1. **Why** — the problem from its source, quoted, then restated plainly.
2. **At a glance** — the file map grouped by purpose: added, modified, removed, with line counts.
3. **Before → after** — per changed flow, the base and head diagrams side by side, or one diagram marked with
   `new`, `changed`, `removed`; behaviour as given / when / then rows, base vs head.
4. **Contracts** — base → head as code blocks.
5. **Reach** — a table: consumer, the changed symbol it uses, whether the PR touches it.
6. **Tests** — a table: test, status (added / changed / removed), what it asserts.
7. **Walkthrough** — the diff itself, every hunk, in reading order: grouped by purpose, foundations first
   (types, contracts), then the logic built on them, then its callers, tests last. Each stop:
   - a heading `### <what it does> — path::symbol`;
   - the hunk verbatim as a `diff` block, with its location, evidence id, and PR on the fence line —
     ```` ```diff src/rules.rs E7 PR #42 ```` — and its `@@` headers kept, so line numbers resolve;
   - notes by cohesive chunk, per writing.md § Rules — a hunk often holds several, and each one worth explaining
     gets its own note: head lines `L250-L258 — …`, base lines for a pure deletion `-L40-L52 — …`. Each says what changed, why, and what it means for the flows above, linking back to them.

   Generated, vendored, and lockfile hunks get one line each instead of a stop. Every changed file appears as a
   stop or on that list.

## Re-run

When the PR head has moved since the explanation was rendered, update only the delta. Diff the explained head
against the new one; re-gather evidence for each ledger item whose cited file changed and for each file the delta
adds; update the sections citing them and the Walkthrough stops the delta touches. Record the new head in `Request` and the delta in `Status`. Then verify the changed sections
(SKILL.md step 4) and re-render (step 5).
