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
target checkout. It is located by `--plan <path>`, or by `--target <repo>` and `--slug <slug>`, or by `--target <repo>` alone (that
repository's current plan), or — with none — by the current plan of the git repository holding the working
directory, `$REVY_PLAN_HOME/current.d/<RepoKey>`; only outside any git repository does it fall back to the global
`$REVY_PLAN_HOME/current`. `init` writes both. A failed lookup names the file it read (the repository's, or the
global one) and exits 2. CLI writes go
through a temporary file plus atomic rename, under a `<plan>.lock` file lock — exclusive for the
load→apply→save window of `apply`/`format`, shared for a read.

A plan may span several repositories. The primary (`init --target`) keys the plan home and is cited unprefixed;
each other repository is declared with an alias (`init --repo ALIAS=PATH`, or later `reopen exploration --repo
ALIAS=PATH`) and captured like the primary into `repositories: {alias: Repository}`. The primary's alias is
`--alias`, else derived from its directory name, and is stored as `repository_alias` only when it differs from
that default. A single-repository plan carries neither field and loads, round-trips, and fingerprints unchanged.
Aliases are kebab-case (`^[a-z0-9]+(-[a-z0-9]+)*$`), distinct from the primary's and from each other, and no two
name the same checkout. Every cited repository file — an evidence `Location.path`, a textual `scope`, an MR's
`files`, a changed symbol's `path`, a verify command's `cwd`, a `tree` diagram entry's `path` — is `path`
(primary) or `alias:path`; the prefix is an alias only when the
text before the first `:` is alias-shaped. A textual `scope` may also be `.` / `alias:.`, the whole repository.
Writing the primary's own alias as a prefix is rejected (one canonical form), as is an undeclared alias (the
error lists the declared ones). Transcript references stay plan/store-relative.

## Run states

`status` reports exactly one of eight run states; the ready frontier is derived from it.

| State | Meaning |
|---|---|
| `awaiting_capability_approval` | The capability report is written; the user must approve it before substantive work. Skipped only at `init`, when the report has no proposals and a capability proving `semantic` evidence: the plan then enters `exploring` with `approval: auto` recorded. Every later report — a revised `capability_report` (after a `request_revision`, whose instruction is kept as `revision_request` and shown in the `capability_report` packet's context) or a mid-exploration `propose_capability` — waits for the user. |
| `exploring` | Gathering evidence against the target repository. |
| `awaiting_user_decision` | A design brief awaits the user's decision. |
| `drafting` | Authoring the candidate plan over the collected evidence. |
| `reviewing` | A review wave is dispatched against the current candidate. |
| `correcting` | A correction answers the wave's findings. |
| `awaiting_review_decision` | The wave could not clear itself; the user must rule on the open findings. |
| `final` | The plan is accepted; `reviewing`/`correcting`/`awaiting_review_decision` loop per wave until a wave clears into this state. |

Two commands move a plan outside the packet flow, each under the same lock as `apply`:

- **`reopen exploration`** (any state after capability approval) returns to `exploring`/collecting over the
  current exploration context, capabilities kept. The brief, decisions (with the user's answers), and any draft
  are carried as `lineage.prior_design`: the next `design_brief` may `--patch` the prior `{brief, decisions}`,
  and a user decision returned with the same question and the same set of alternatives keeps its prior answer
  instead of being asked again. **`reopen draft`** (any candidate-bearing state) returns to `drafting` with the
  prior draft as the `draft` packet's `--patch` subject (`{content, delivery}`). Each reopen records
  `{target, reason}` (shown by `status` under `reopens`) and keeps the set-aside candidate's review history and
  revisions — and, from `final`, its review comments — for the render timeline; the next candidate starts review
  at wave 1. Plans whose review states predate recorded capabilities cannot reopen. `reopen exploration
  --recapture` (for a moved checkout) first re-captures every declared repository's identity into the plan and
  clears the `target` of each carried evidence item that may omit it, so the next exploration apply re-verifies
  its anchors against the current checkout and stamps the new identity; runtime and history evidence keep their
  required identities and must be re-observed. Until a new draft exists, `render` previews the set-aside
  candidate under a `DRAFT — reopened` watermark.
- **`revise`** (`reviewing`, `correcting`, `awaiting_review_decision`, `final`) replaces the candidate with a
  user/orchestrator revision built by patching `{"candidate": …}`; its evidence goes through the same runtime
  stamping, anchor, and approved-tool-failure checks as a correction, it is re-proven, recorded as
  `Revision {wave, note, addresses}` (`addresses` may name any finding in the candidate's history, advisories
  included), and the plan enters `reviewing` at the wave after the last recorded review round. A revision must
  change the candidate and discharge what the state owes: from `correcting`, `--addresses` must name every
  `must_fix` finding; from `awaiting_review_decision`, every unresolved finding — that revision records the
  escalation `cause` and keeps the automatic-wave budget counting from where it stood, so it cannot replace the
  user's ruling with fresh automatic waves. From `final`, each accepted finding the revision does not address is
  kept as `carried_accepted` and accepted again at the next `final` (a later revision addressing it, or a user
  ruling on the same finding id, settles it). A revision may change a user-owned decision's answer or
  alternatives; each such change is recorded as `decision_changes: [{decision, field, before, after}]`
  (`field`: `answer`, `alternatives`, `added`, `removed`), shown in `status` and the render timeline.

New run-state fields (`lineage`, `carried_answers`, `approval`, `revision_request`, a reopen's `comments`, a
revision's `cause`/`carried_accepted`/`decision_changes`, a decision's `note`) default when absent and are omitted
when empty, so older `plan.json` files load and keep their digests.

## Work packets (the `status` contract)

`status --format json` is the workflow interface: `{ "state", "packets": [...] }`, plus `wave` in the review
states, `accepted_findings` at `final`, `capability_approval` (`user` or `auto`) once capabilities are approved,
`reopens` after a reopen, and `latest_revision` (`{wave, note, addresses}`, plus `cause` and `decision_changes`
when present) after a revision. Each ready packet carries a stable `key`, a closed `kind`, a typed
`subject`, an exact `mandate`, its `context`, and a `schema_ref` (its result schema, inlined under `--schema`).
A final plan emits no packets. The ten work kinds are coupled to the `WorkKind` enum, not hand-listed:

`capability_report`, `capability_approval`, `exploration`, `design_brief`, `user_decision`, `draft`,
`integrated_review`, `specialist_review`, `correction`, `review_decision`.

A `capability_approval` decision may omit its `report` digest: it then binds to the report of the packet it
answers; a digest given must match. A user's answer `note` is kept on the answered decision. After a `reopen
draft`, the `draft` packet's context lists the prior draft's addressable ids (requirements, criteria, risks, MRs,
changed symbols, sections by index) and the patch subject shape with an example.

The `exploration` packet's context carries `target_identity` (the primary's `{revision, dirty_digest}`),
`primary_repository` (its alias), and `repositories` (`{alias: {root, identity, cite_as}}` for every repository).
An evidence item's repository is the one its anchors and scopes cite; anchors resolve against that repository's
root. Evidence that omits `target` is stamped with that repository's identity at ingest when a fresh capture of
that checkout still matches the plan, and rejected naming the repository and both revisions when the checkout
moved (the error suggests `reopen exploration --recapture`; the same items are not reported again as ungrounded).
Anchorless evidence that omits `target` is stamped with the primary's identity only in a single-repository plan;
with several repositories it must carry an explicit `target`. An explicit `target` that is not the cited repository's identity is
`STALE-MODEL`; anchorless evidence (documentation, runtime, history) may carry any declared repository's
identity. Evidence citing two repositories is rejected naming both — split it per repository. Grounding
runs on every exploration apply and the nine-lane coverage checklist (errors name lanes by wire value, e.g.
`data_at_rest`) on the exploration that completes
(`next: complete`) — the same proofs a draft re-runs. An approved tool counts as used when evidence provenance
(or a runtime command's program) names its proposal id or name, case-sensitively, or when the exploration records
it in `tool_failures: [{proposal, reason}]`; failures stay on the exploration context for the candidate,
reviewers, and render. The result schemas publish the id `pattern` and the `alias:path` / scope `pattern`s. A capability report supplied by a `capability_report` packet or an exploration's `propose_capability`
may omit `repository`; the CLI stamps the plan's identity, as `init` does.

A runtime-evidence `artifact` (`{path, digest}`) verifies against the plan's own `transcripts/<path>`; when that
file is absent or does not match, against the shared content-addressed store `$REVY_PLAN_HOME/transcripts/<digest>`,
which `transcript add` fills — so a restarted plan needs no hand-copied transcripts. A failure names both
lookups.

Every `--patch` (and `revise --patch`) path may use `@<key>` segments: under an array, `@<key>` selects the one
element whose identity — the first of `id`, `subject`, `symbol`, `lane` it carries — equals `<key>`, e.g.
`/candidate/content/risks/@risk-cache/statement`; a selector matching no element or several is an error naming
it. The `correction` packet's context carries the patch subject shape and an example.

A packet carries only its own mandate and schema fragment; independent ready packets may be executed
concurrently, and `status` exposes only the current frontier and its immediate blockers — never later stages
whose dependencies are incomplete. Each packet's tool-heavy work runs in a fresh, context-minimal subagent
that returns only the typed result.

## Commands

The binary is `revy-plan` with flattened top-level subcommands.

| Command | Does / enforces |
|---|---|
| `init --target --slug --capabilities [--alias NAME] [--repo ALIAS=PATH]...` | Creates `plan.json` atomically from a capability report, capturing the primary repository's identity and stamping it into the report (`repository` may be omitted) and capturing each `--repo` checkout under its alias; records the plan as current; refuses to clobber an existing plan; rejects an invalid slug, a malformed or colliding alias, and a plan home inside any declared checkout. |
| `schema [section] [--packet KIND]` | Prints the full generated JSON Schema, one named `$defs` section, or one packet kind's result schema. The only format authority outside work packets; generates no `OneOrMoreN` definitions. |
| `format` | Rewrites `plan.json` in canonical ordering under an exclusive lock. |
| `status [--format] [--schema]` | Derives the ready/blocked frontier without mutating the plan; `--schema` inlines each packet's result schema. |
| `apply [--work KEY\|KIND] (--result FILE \| --patch FILE)` | Decodes the result for the selected ready packet against its schema, checks it, and advances the plan atomically. `--work` names a work key or an unambiguous packet kind, and may be omitted when exactly one packet is ready. `--patch` is a JSON array of RFC 6902 ops over the packet's subject (the prior exploration; the current candidate for a correction; after a reopen, the prior brief and decisions for a `design_brief` and the prior draft for a `draft`), with `@<key>` element selectors. `-` reads either from stdin. A rejection lists every runtime-check and transition problem, each with its JSON pointer (a problem with the whole result prints bare). Success prints one line, `applied <kind> <key> to <plan path> → <state>`. |
| `check [--work KEY\|KIND \| --packet KIND] (--result FILE \| --patch FILE)` | The same decode, runtime check, and transition as `apply`, as a dry run: prints every problem (or the state the plan would enter), never writes; exit `0` clean, `1` rejected. |
| `gate [--allow-accepted]` | Succeeds only at `final`; fails (exit 1) before `final` or when a final plan carries accepted unresolved findings, unless `--allow-accepted`. A malformed plan is a usage error (exit 2). |
| `render [--out DIR] [--brief]` | Writes `plan.md` and standalone `plan.html` from the typed candidate; never mutates the plan. Runs at any candidate-bearing state, and after a reopen previews the set-aside candidate (`DRAFT — reopened`); otherwise fails saying no candidate or retained draft exists. |
| `serve [--port] [--brief] [--plan]` | Serves the rendered plan at one stable loopback URL, re-rendering on edit; reports a bind failure as a usage error (exit 2) and a broken plan in-page rather than exiting. |
| `reopen exploration\|draft --reason TEXT [--repo ALIAS=PATH]... [--recapture]` | Sends the plan back to `exploring` (after capability approval) or `drafting` (from a candidate-bearing state), keeping approved capabilities and carrying the prior design for `--patch`; records the reason. `--repo` (exploration only) declares and captures more repositories; `--recapture` (exploration only) re-captures every repository's identity and clears carried evidence targets for re-stamping. Unavailable from other states (exit 1). Prints `applied reopen <target> to <plan path> → <state>`. |
| `revise --patch FILE --note TEXT [--addresses FINDING...] [--check]` | Patches `{"candidate": …}` into a revised candidate, runtime-checks and re-proves it, records the revision, and re-enters `reviewing` at the wave after the last recorded review round. `--addresses` must cover the state's obligations (every `must_fix` while correcting, every unresolved finding when escalated). `--check` is the dry run: same problem list, nothing written. `-` reads the patch from stdin. Prints `applied revise to <plan path> → <state>`. |
| `transcript add FILE` | Copies FILE into the shared store `$REVY_PLAN_HOME/transcripts/<sha256>` (idempotent) and prints the `{path, digest}` reference to cite as a runtime evidence `artifact`. Needs no plan. |

Commands that locate a plan (`reopen` and `revise` included) take `--plan`, or `--target` with `--slug`, or
`--target` alone, or fall back to the working directory's repository's current plan (the global one outside any
repository).

Exit codes: `0` success; `1` a checked failure (a non-final gate, a rejected apply, reopen, or revise); `2` a
usage/structural error.

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
finding, which sends the wave straight to a user ruling. The three-wave automatic budget counts waves since the
latest `revise` that did not answer an escalation (or since wave 1). A correction may reword a user-owned decision — its question, consequences, or
ruled-out reasons — but never add or remove one or change its answer or set of alternatives
(`UserDecisionImmutable` names which); such a change is made only with the user's approval, through `revise`,
which records it for the user to see. Exactly two states block the gate: a thread whose
latest disposition is a challenge, and a blocking open question.

## Rendering

The renderer consumes typed entities directly; nothing is hand-written and nothing is read back out.

- **Authored content first**: brief and behavior, orientation, sections, decisions, delivery (merge requests
  grouped into dependency stages — the one rendering of the delivery graph), and unresolved facts.
- **Derived figures** are computed from the validated artifact and follow in **one appendix** — a single
  collapsed disclosure in HTML, one trailing `Appendix` heading in Markdown: the evidence map (claim and evidence
  limitations folded onto it), impact areas (covered lanes on each subject's row, set-aside lanes collapsed by
  reason; approved tools that failed listed under the evidence map), changed symbols, the as-is call graph, the
  review timeline (with a marker for each reopen and revision, earlier candidates' waves first, each listing the
  review comments it set aside and the user-decision changes it made), and resolved
  discussion. No figure
  re-lists entities another section already renders. `render --brief` and `serve --brief` omit the appendix.
- **Authored figures** are model-authored `Section`s of `Code` and `Figure` blocks; a `Figure` carries one
  closed `Diagram` kind (`layers`, `flow`, `seq`, `er`, …) chosen for the shape of the content. Figures center
  and widen only when content requires it.
- A candidate rendered before `final` carries a **`DRAFT — <state>` watermark**; accepted unresolved findings
  render under an `ACCEPTED WITH <n>` banner in both formats.

Markdown is generated for portability; standalone HTML is the primary visual review surface, served at one
stable loopback URL when reachable and delivered as the standalone artifact otherwise.
