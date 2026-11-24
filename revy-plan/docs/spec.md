# revy-plan — spec

The planning domain behind the skill: the canonical artifact, the run states, the work-packet contract, the
review loop, and what each command enforces.

## Principles

1. **One editable artifact.** `plan.json` is the sole source of planning truth. Generated Markdown, HTML, and
   diagrams are projections — never parsed back, never edited as sources. The artifact rejects unknown fields.
2. **CLI drives, model plans.** The CLI is the deterministic workflow driver: it derives the ready work, owns
   the result schemas, advances state atomically, and gates the finish. The model authors facts, design,
   decisions, and review rulings; it is the planner and the judge, never the sequencer.
3. **Evidence first.** Every material claim links to evidence from the best available method — code
   intelligence, runtime, prototype, or, as the universal floor, reading source and grep. The method is named;
   a text search never impersonates semantic proof.
4. **Derived, not stored.** Readiness, waves, and blockers are derived from the current artifact on every
   `status`; nothing persists mutable job state. The plan is the memory — `status` outranks recollection.
5. **Any harness.** The skill is portable; the CLI calls no model, no harness, and no code-intelligence tool.
   It decides what work is valid and what proves it complete; the harness decides where and when to run it.

## Artifact

`plan.json` lives in an external plan home (`~/.revy-plan/…`, overridable by `REVY_PLAN_HOME`), never inside the
target checkout. It is located by `--plan <path>`, or by `--target <repo>` and `--slug <slug>`. CLI writes go
through a temporary file plus atomic rename, under a `<plan>.lock` file lock — exclusive for the
load→apply→save window of `apply`/`format`, shared for a read.

## Run states

`status` reports exactly one of eight run states; the ready frontier is derived from it.

| State | Meaning |
|---|---|
| `awaiting_capability_approval` | The capability report is written; the user must approve it before substantive work. |
| `exploring` | Gathering evidence against the target repository. |
| `awaiting_user_decision` | A design brief awaits the user's decision. |
| `drafting` | Authoring the candidate plan over the collected evidence. |
| `reviewing` | A review wave is dispatched against the current candidate. |
| `correcting` | A correction answers the wave's findings. |
| `awaiting_review_decision` | The wave could not clear itself; the user must rule on the open findings. |
| `final` | The plan is accepted; `reviewing`/`correcting`/`awaiting_review_decision` loop per wave until a wave clears into this state. |

## Work packets (the `status` contract)

`status --format json` is the workflow interface: `{ "state", "packets": [...] }`, plus `wave` in the review
states and `accepted_findings` at `final`. Each ready packet carries a stable `key`, a closed `kind`, a typed
`subject`, an exact `mandate`, its `context`, and a `schema_ref` (its result schema, inlined under `--schema`).
A final plan emits no packets. The ten work kinds are coupled to the `WorkKind` enum, not hand-listed:

`capability_report`, `capability_approval`, `exploration`, `design_brief`, `user_decision`, `draft`,
`integrated_review`, `specialist_review`, `correction`, `review_decision`.

A packet carries only its own mandate and schema fragment; independent ready packets may be executed
concurrently, and `status` exposes only the current frontier and its immediate blockers — never later stages
whose dependencies are incomplete. Each packet's tool-heavy work runs in a fresh, context-minimal subagent
that returns only the typed result.

## Commands

The binary is `revy-plan` with flattened top-level subcommands.

| Command | Does / enforces |
|---|---|
| `init --target --slug --capabilities` | Creates `plan.json` atomically from a capability report, capturing repository identity; refuses to clobber an existing plan; rejects an invalid slug. |
| `schema [section] [--packet KIND]` | Prints the full generated JSON Schema, one named `$defs` section, or one packet kind's result schema. The only format authority outside work packets; generates no `OneOrMoreN` definitions. |
| `format` | Rewrites `plan.json` in canonical ordering under an exclusive lock. |
| `status [--format] [--schema]` | Derives the ready/blocked frontier without mutating the plan; `--schema` inlines each packet's result schema. |
| `apply --work KEY (--result FILE \| --patch FILE)` | Decodes the result for the named ready packet against its schema, checks it, and advances the plan atomically. `--patch` is a JSON array of RFC 6902 ops over the packet's subject (the prior exploration, or the current candidate for a correction). |
| `gate [--allow-accepted]` | Succeeds only at `final`; fails (exit 1) before `final` or when a final plan carries accepted unresolved findings, unless `--allow-accepted`. A malformed plan is a usage error (exit 2). |
| `render [--out DIR]` | Writes `plan.md` and standalone `plan.html` from the typed candidate; never mutates the plan. Runs at any candidate-bearing state. |
| `serve [--port] [--plan]` | Serves the rendered plan at one stable loopback URL, re-rendering on edit; reports a bind failure as a usage error (exit 2) and a broken plan in-page rather than exiting. |

Exit codes: `0` success; `1` a checked failure (a non-final gate, a rejected apply); `2` a usage/structural error.

## Review loop

The CLI *derives* the review set from the candidate — an **integrated review** always, plus a **specialist
review** for each impact shape that triggers one — and emits it as one concurrent frontier; the skill
dispatches every member together. Each review runs in a subagent that did not draft the plan and returns
**findings only** — it never resolves, triages, or edits the plan. The integrated review additionally owes a
verdict for every `not_considered` lane and a consumer check for every changed symbol.

A reviewer files findings; a `correction` then carries one `triage` per material finding, with a typed outcome:

| Outcome | Means |
|---|---|
| `Fix { affected }` | Did what the finding asked; `affected` names every region the fix moved (a `Fix` touching nothing is rejected). |
| `Reject { rationale }` | The finding is wrong; it is not applied. The evidence for refusing lives in `rationale`. |

When a wave cannot clear itself the CLI escalates to the user, whose `Ruling` is the only other outcome, one per
unresolved finding: `MustFix` (the next wave must fix it), `Dismiss` (dropped, kept in history), or `Accept`
(carried to `final` unresolved and shown in the render). A finding may `reraise` an earlier wave's rejected
finding, which sends the wave straight to a user ruling. Exactly two states block the gate: a thread whose
latest disposition is a challenge, and a blocking open question.

## Rendering

The renderer consumes typed entities directly; nothing is hand-written and nothing is read back out.

- **Derived figures** are computed from the validated artifact: the evidence map, impact areas, changed symbols,
  the review timeline (folded into a disclosure once a wave exists), and the delivery breakdown.
- **Authored figures** are model-authored `Section`s of `Code` and `Figure` blocks; a `Figure` carries one
  closed `Diagram` kind (`layers`, `flow`, `seq`, `er`, …) chosen for the shape of the content. Figures center
  and widen only when content requires it.
- A candidate rendered before `final` carries a **`DRAFT — <state>` watermark**; accepted unresolved findings
  render under an `ACCEPTED WITH <n>` banner in both formats.

Markdown is generated for portability; standalone HTML is the primary visual review surface, served at one
stable loopback URL when reachable and delivered as the standalone artifact otherwise.
