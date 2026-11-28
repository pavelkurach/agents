---
name: revy-explain
description: Explain how something works, a task, a pull request, a review report's findings, or code as an evidence-backed visual document; follow-up questions land in its FAQ. Invoked as /revy-explain <question | task | PR | report | code>.
disable-model-invocation: true
argument-hint: "<question | task | PR | report | code>"
---

# revy-explain

Produce an explanation a **cold reader** understands in full — someone who has never opened this code, this
product, or this conversation. **Picture first** (writing.md § Rules). Concise in words, exhaustive in coverage:
cut words, never an actor, a step, a branch, or a failure path. Every claim rests on evidence.

The reader arrives cold and leaves as the system's **owner**: give them every row of § Owner's depth, so they can
decide about the system, not only follow it. Build their model from **first principles** (writing.md §
explanation.md). Dive to the bottom of every part the subject touches; a part stays shallow only when `Request`
records it out of scope.

You coordinate; subagents do the tool-heavy work. Brief each one with only what its task needs — the goal, the
paths to read, the repo root, an output path — and require a short return: the result, how it was verified, the
gaps, open questions. Run independent subagents together. Explorers can run on a cheaper model; drafting and
verifying need a strong one.

## Modes

| Mode | Request | Shape |
|---|---|---|
| `how-it-works` | a flow, feature, or concept — *how authentication works in this surface* | [how-it-works.md](references/how-it-works.md) |
| `task` | a feature to build or a bug to fix — a ticket, an issue, a description | [task.md](references/task.md) |
| `pull-request` | a PR/MR number or URL | [pull-request.md](references/pull-request.md) |
| `review-report` | a code review report whose every finding needs explaining | [review-report.md](references/review-report.md) |
| `code` | a file, module, type, or function — *explain `Session::refresh`* | [code.md](references/code.md) |
| `follow-up` | any question about an existing explanation | § Follow-ups |

Every mode also covers § Owner's depth, scoped to what its subject touches.

`task` and `pull-request` map the **terrain, not the route**: the solution, the fix, and the verdict stay the
reader's. A fact the evidence proves — a cause, a consumer the change breaks — is terrain, shown with its ids.

## Owner's depth

| Enumerate | Scope |
|---|---|
| Rationale | why it is built this way — commit messages, PR and issue threads, ADRs, design docs; a reason no source gives goes under Gaps, never invented |
| Invariants | what it guarantees, and the code, type, or check that enforces each — or that nothing does |
| Limits | sizes, rates, timeouts, concurrency, resource bounds, and what happens past each |
| Intricacies | behaviour that surprises — edge cases, ordering, defaults, implicit coupling, silent fallbacks |
| Trade-offs | what the design gave up, and what it got for it |

## Files

Directory: `~/.revy-explain/explanations/<slug>/`. Slug: a kebab-case noun phrase for the subject, ≤ 40 chars.

| File | Holds |
|---|---|
| `explanation.md` | the explanation — content and Mermaid diagrams, citing evidence ids |
| `log.md` | `Status` (current step, what remains; update it after every step), `Request` (verbatim, mode, scope), `Evidence` (the ledger, per [evidence.md](references/evidence.md)) |
| `report.md` | `review-report` only: the report, verbatim |
| `render/` | the renderer's output |
| `scratch/` | subagent outputs, worktrees |

## Steps

### 1. Frame

First read the `Request` of every existing explanation. A request naming a slug or path, or a subject one covers,
is a follow-up; ask the user when two fit or the fit is unsure. A match whose `Status` shows step 5 done goes to
§ Follow-ups; any other match resumes from its `Status`.

Pick the mode and write `Request`. Read `renderer` from `~/.revy-explain/config.toml`. Ask the user, in one
batch, each with your recommended default:
- what changes what gets explained — which surface, which repository, which of two features sharing a name;
- when `renderer` is absent: which skill renders explanations, recommending `revy-render` — the conventions in
  writing.md follow it. Save the answer as `renderer = "<skill>"`.

**Done:** mode and scope are fixed, every ambiguity that changes the scope has the user's answer, and
`renderer` is set.

### 2. Explore

Snapshot the explained revision first, before reading any code, per evidence.md § Recording; the owner may keep
editing while you work. Every explorer and verifier reads that snapshot's worktree.
Inventory capabilities: code intelligence per language touched, the project's own run and test commands,
structured and text search. Then gather evidence per [evidence.md](references/evidence.md) against the mode's
**Enumerate** table. Dispatch one explorer per independent area — a service, a repository, a flow stage, a
finding — or per open question; read a small area yourself. Brief each explorer to resolve definitions,
references, and callers with the inventoried code intelligence, falling back to `via:grep` only where it has
none, and to return its findings as ledger lines per evidence.md § Recording, so they merge without re-keying.

**Done:** every row of the mode's Enumerate table and of § Owner's depth has ledger lines covering it in full, or a
Gaps entry saying what is missing.

### 3. Draft

Write `explanation.md` per [writing.md](references/writing.md), with the mode's **Body** as its body.

**Done:** every section is present, or one line says why it is empty; every term is defined where first used;
every claim and every diagram edge cites an evidence id.

### 4. Verify

Dispatch a fresh verifier with the paths of `explanation.md`, `log.md`, [writing.md](references/writing.md), and the
repo root. It:
- checks every claim and diagram edge against its cited source, and every snippet character for character against
  the explained revision;
- parses every diagram with the lint in writing.md § Presentation, else `mmdc`; a failing diagram is a must-fix, and
  with no parser it says so;
- reads as the cold reader, listing each term used before its definition, each skipped step, each passage breaking
  writing.md § Rules;
- tags each finding **must-fix** (wrong or unbacked claim, wrong edge, undefined term, skipped step, failing
  diagram) or **polish**, and each claim `verified`, `wrong`, or `unsettled`.

Fix every must-fix and each polish item confined to one passage. A fresh verifier re-checks only the changed
sections; repeat until one returns no must-fix. After round three, fix and self-check the remaining mechanical
must-fixes and show the user the rest. Write the trust line (writing.md § explanation.md) from the final tags.

**Done:** the last verifier returns no must-fix, or after round three each remaining must-fix is fixed and
self-checked or shown to the user; every `unsettled` claim sits under Gaps; the trust line matches the final tags.

### 5. Render

Dispatch a subagent to invoke the `renderer` skill, writing into `render/`. When its Skill tool does not list the
renderer, it reads `~/.claude/skills/<renderer>/SKILL.md` and follows it. Give it:
- `explanation.md` as the document;
- `log.md` as the evidence source (§ `Evidence`) and its § `Request` link base, as `<web-url>@<link-commit>` per
  repository;
- each repository root, with its alias when there are several;
- the requests in [writing.md](references/writing.md) § Presentation.

Tell it `explanation.md` is the content, rendered verbatim: every item and every diagram node and edge as written;
it decides only the look. When the skill is missing or fails,
report the error and ask the user for another.

**Done:** every section, item, diagram node, and edge of `explanation.md` appears in the render, and the user has
its path, the Answer line, and the Gaps.

## Follow-ups

Every question the user asks about an explanation — later in this session or in a new one — becomes an FAQ
entry.

1. Compare the code in scope now with the revision in `Request`. When it has moved, tell the user what moved and
   ask: answer at the explained revision (rebuilt per evidence.md § Worktrees), or refresh the explanation first — for a pull request, per
   pull-request.md § Re-run.
2. Answer from the existing ledger first. Gather new evidence per § Steps 2 only for what it lacks, on the Enumerate
   rows the question touches, continuing the existing evidence ids.
3. Append the entry to `explanation.md` § FAQ per [writing.md](references/writing.md).
4. When the answer corrects or extends a body claim, fix the body too, and the entry says what changed.
5. Verify the entry and every changed section (§ Steps 4), then re-render (§ Steps 5).

**Done:** the entry and every body fix appear in the render, and the user has the answer in one or two lines plus
the render's path.
