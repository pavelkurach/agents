# Review loop

What you owe each user comment, and the bar for disagreeing. The CLI owns when a review packet runs and
which exact reply advances it; this file is the judgement inside one.

## Independent review

Every review runs in a subagent that did not draft the plan — an author shares the assumptions that
produced it, which is not review. Each returns findings only: it reads, it does not edit the plan,
triage its own findings, or apply a fix. The CLI *derives* the review set from the candidate — an
integrated review always, plus a specialist for each impact shape that triggers one — and emits it as
one concurrent frontier; the skill dispatches every member together, not one at a time.

**The `consistency` lens cross-checks the plan against itself** — brief vs target behavior vs decisions
vs uncertainties vs acceptance criteria — for internal contradiction: an uncertainty still `open` though
a decision already settled it, a target the decisions do not deliver, a criterion nothing verifies. This
is a review obligation, not a parse gate: natural-language contradiction is undecidable at parse time, so
name it and file a `consistency` finding rather than trusting a type to have caught it. The referential
half that *is* checkable — an accepted uncertainty citing its deciding `decision` — is already enforced
when you wire `UncertaintyDisposition::AcceptedByUser`.

## The body and the conversation

**The body is the plan as it now stands** — no comments, no replies, no history. The conversation lives
in a discussion section at the bottom: threads, oldest first, each anchored to what it was about.

Any blockquote in the body is a comment — no label, no prefix. **You classify, they don't:** infer
question, objection or preference from the text; a `label:` prefix, if they use one, overrides your
inference. **Placement is the anchor** — a comment under a table row is about that row. Comments given
in chat are transcribed into the file first, at the place they concern, before anything else happens.

A blockquote that was never addressed to the plan — placeholder text, a stray paste — gets no thread:
record it verbatim in a `Not comments` list with where it sat and why, and delete it from the body. Be
strict: a comment you merely disagree with is `challenged`, and one you do not understand is a question
you must ask.

## Dispositions

**In a review wave the disposition is a *typed* outcome the CLI ingests**, never a free-text label. A
reviewer only files findings; a `correction` then carries one `triage` per material finding, and its
outcome is one of two:

| Prose | Typed outcome | Means |
|---|---|---|
| `applied` | `Fix { affected }` | Did what the finding asked; `affected` names every region whose fingerprint the fix moved (a `Fix` that touches nothing is rejected) |
| `challenged` | `Reject { rationale }` | The finding is wrong. **Does not apply it.** The `right:`/`wrong:`/`falsifier:` lines below go *inside* the `rationale` |

When a wave cannot clear itself the CLI escalates to the user, whose `Ruling` is the only other
outcome — one per unresolved finding:

| Prose | Typed verdict | Means |
|---|---|---|
| `overridden` | `MustFix` | A `Reject` the user ruled against; the next wave must fix it |
| `sustained` | `Dismiss` | A `Reject` the user ruled for; the finding is dropped (kept in `history`) |
| — | `Accept` | The user accepts the finding unresolved; it is carried to `final` and shown in the render |

**`reraises`.** A finding may carry `reraises: <finding-id>`, naming a finding that an earlier wave
rejected and this one re-raises. A reraise sends the wave straight to a user ruling instead of another
automatic correction: a rejection the reviewers keep re-filing is a disagreement only the user settles.

The prose labels `answered`, `applied-differently`, `deferred`, `resolved`, and `corrected` are **not**
workflow outcomes — the CLI has no field for them, so never try to `apply` one. They remain the
vocabulary for `serve` comment threads on a `final` plan (the discussion below), where a user's
blockquote — not a reviewer's finding — is what you dispose of:

| Serve disposition | Means |
|---|---|
| `answered` | Replied to a question; the body may not change |
| `applied-differently` | Addressed the underlying problem another way |
| `deferred` | Out of scope, or decided by a section this comment does not belong to — name which |
| `resolved` | Was `deferred`; the user has now ruled — say which way |
| `corrected` | The comment states something about the document that reading the document refutes. Quote the text that refutes it. No body change, and it does not block |

`corrected` is narrow: only a claim about the document's own contents, settled by quoting it. A
disagreement about the design is `challenged` however wrong it looks. Never force a question into
`applied`.

## A thread per round, appended

A disposition can change — `challenged` to `overridden`, `deferred` to `resolved`. **The thread appends
a new block rather than rewriting the old one** — round, then disposition, then the reply. The latest
block is the live state; the ones above it are why a comment was refused before it was ruled on.

```markdown
> just always sort by priority, simpler

**round 1 · challenged** · always-sorting silently changes output for every current caller
  right: nothing in the repo pins `list`'s order — no test asserts it (via:grep tests/)
  wrong: `scripts/daily.sh:12` pipes `list` and indexes by line position (via:read)
  failure: daily.sh treats line 1 as the oldest task; a priority sort makes line 1 arbitrary
  falsifier: daily.sh is dead, or does not index by position

> daily.sh is being retired this quarter, sort it

**round 2 · overridden** · applied; daily.sh breakage is now an acceptance criterion
```

**Commit each review's raw findings before revising the plan in response to them** — what came
back, not a summary. The revision is then checked against a record nothing can rewrite after the fact.

## Ownership is checked before the merits

**A comment deciding something a later section owns is `deferred`** — a comment that would win still
defers, carrying its case forward so that section inherits it. Judging one option with no alternatives
on the table is how a strawman becomes a decision. This runs first and settles the thread. **Defer the
decision, keep the finding:** a comment can belong to another section *and* be right about a defect
where it sits — fix what it exposed here, and say which half went where.

## Counter-case — owed by `challenged`, `applied-differently` and `deferred`

Agreeing needs no defence. **Refusing does.** Each of the three carries two lines:

- `right:` — the strongest case the comment is **correct**, written as the reviewer would write it.
- `wrong:` — the strongest case it is **not**.

**If `right:` is stronger, the disposition is `applied`** — whatever you believed before writing it.
`right:` cites something the plan already contains: a symbol, a criterion, a decision row. An uncited
`right:` counts as unwritten, and an unwritten `right:` forces `applied`. `applied-differently` needs
its counterfactual evidenced against a **sibling in the codebase** — where the shape already exists and
already misbehaves, not imagination.

## The bar for disagreeing

> **No evidence chain → no challenge.**

Every claim carries a `via:` marker ([craft.md](craft.md)). One consequence line, chosen by the weakest
frame the refusal rests on — not by your last lookup:

| Weakest frame | Line owed |
|---|---|
| `via:lsp` · `via:runtime` · `via:prototype` · `via:doc` · `via:read` | `failure:` — concrete inputs or state producing a wrong output or a crash |
| `via:tool` · `via:grep` | `change-cost:` — when *<requirement>* arrives this shape forces *<edit>* at *<N>* sites and the alternative does not. `N` is a real count |
| `via:inference` | none — you do not have a challenge |

A `falsifier:` line closes every challenge. **If no consequence line can be written, apply the
comment.** Recording it as a blocking open question is the same refusal relabelled; an unevidenced
objection may not stop a gate.

**Two comments that pull opposite ways** are not a challenge — no evidence can settle it — and not
grounds to pick one. Quote both, apply neither, ask which governs as a blocking open question.

**Prohibited:** strawmanning; re-challenging after a ruling; dropping evidenced findings.

## Challenges to the request

The agent challenging the *request* closes the same way a challenged comment does, and each open one
blocks until the user rules. `right:` and `wrong:` read against the request here: `right:` is the
strongest case the request is correct as written, and still cites something the plan already contains —
the code, the criterion, the constraint that makes the request sensible. A challenge with a stronger
`right:` is not a challenge: drop it and say what convinced you.

## Unresolved state and invalidation

Each unresolved item has one home: a fact the evidence could not establish; a blocking or informational
open question; a deferral or overridden challenge carried across rounds; a deliberate scope exclusion.
Exactly two states block a gate: a thread whose latest disposition is `challenged`, including each
unruled request-challenge; or a blocking open question.

**Invalidation takes back a gate already passed.** Something contradicting an approved plan reopens it:
say what, clear the approval, run a round, re-approve, then resume. Never amend an approved document in
place. A user directive to proceed is not a ruling — name the open threads and ask.

## Gate report

Present two fixed fields **in chat** — at approval, or a round's end — alongside whatever the CLI's own
`status` or `render` output already shows. These are chat-only: the CLI carries no field for either, so
never try to `apply` them.

| Field | Content |
|---|---|
| `blind spot:` | one approved decision no test can check — [design.md](design.md) § Test seams. Say what closes every decision when there is none |
| `skill contradictions:` | conflicting or ambiguous rules you found in this skill while planning, cited by file and line — [craft.md](craft.md) § Skill contradictions. `none` when you found none |
