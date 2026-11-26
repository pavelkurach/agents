---
name: revy-plan
description: Use for `/revy-plan`, technical specs, design requests, or implementation-plan requests for non-trivial code changes before implementation.
---

# revy-plan

Drive the Rust `revy-plan` CLI. The CLI owns sequencing, work packets, result schemas, completion, subjects, blockers, and rendering. Never restate a schema or mandate it emits — run `revy-plan schema` and `revy-plan status` to read them.

## 1. Locate capabilities

Find, and name by purpose not product: the `revy-plan` binary on `PATH`; the external plan home, where plan.json lives outside the target checkout; filesystem and shell; an isolated review context; the evidence capabilities available now — code-intelligence (LSP, call hierarchy, references), runtime execution, structured and text search; and a delivery path, a reachable loopback URL or a file artifact. No tool product is mandatory. Reading source and grep are the universal floor every method falls back to.

## 2. Init or resume

Inventory the capabilities, write them to `capabilities.json` (omit `repository`; the CLI stamps it), and `revy-plan init --target REPO --slug SLUG --capabilities capabilities.json` creates plan.json and makes it the target repository's current plan, so later commands run from inside that repository need no `--plan` (from elsewhere pass `--target REPO`); when the change reads or touches other repositories, declare each with `--repo ALIAS=PATH` (repeatable; a missed one is added later with `revy-plan reopen exploration --repo ALIAS=PATH`); resume by reading `revy-plan status`. The report must carry a proposal for every missing capability that can be added — a language server for an uncovered language, a code-graph tool — so it never looks complete by omission.

## 3. Present, then wait

When `status` is `awaiting_capability_approval`, present the capability report and wait for the user's exact approved decision; capability approval is required before any substantive exploration, and only the exact reply the packet emits is accepted. Otherwise `init` found nothing to approve (no proposals, code intelligence already available) and recorded `capability_approval: auto`: tell the user in one line that the setup was complete, and proceed. Only `init` approves automatically: a revised report (after the user's `request_revision`, whose instruction the `capability_report` packet shows as `revision_request`) or an exploration's capability proposal always goes back to the user.

## 4. Execute each ready packet

```
revy-plan status
```

For every ready packet, treat its emitted mandate, subject, schema, and completion criteria as the whole instruction. Execute it with the best approved evidence method for its uncertainty; where that method is unavailable or fails, fall back to reading source and grep, and record the weaker evidence honestly. Proactively recommend a higher-value capability when its benefit, cost, side effects, and one limitation are worth stating — recommend, never require. Reviews are derived from the candidate — an integrated review always, plus a specialist for each impact shape that triggers one — not a fixed roster; each runs independently in a fresh, context-minimal subagent — never a full-history fork — briefed only with the candidate artifact path, the review packet, the repo root (and each declared repository's root), a checkpoint path, and findings-only instructions; the whole derived set dispatches concurrently. Reviewers return findings only — they never resolve, triage, or edit the plan; the integrated review owes three things: findings, a verdict for every `not_considered` lane, and a consumer check for every changed symbol. A correction is a `--patch` plus a `triage` that names every material finding.

**Orchestrate; do not absorb.** The main session runs the CLI and the user's turns; each packet's tool-heavy work runs in a fresh, context-minimal subagent — **never a full-history fork**. Exploration (code-intelligence, search, reading source) and drafting (reasoning over that evidence) are the tool-heavy packets: dispatch each to its own subagent. The subagent runs the whole loop and returns only the packet's typed result and its symbol-citations — never raw tool output; hand it the packet's mandate, subject, schema, any input artifact **by path** (never paste evidence), and a checkpoint path it updates so another agent can resume. One subagent per packet, not per query, so a code-intelligence server onboards once. The main session then holds only what §6 reserves for it — statuses, artifact paths, and the decisions the user must make; raw LSP and grep dumps degrade planning judgement and re-cost every turn they sit in context. The
plan is the memory: `plan.json` and `revy-plan status` outrank your recollection — after any context
loss, re-read status before the next decision.

Match the packet to its judgement:

- evidence, bug reproduction, and pushback — [behavior.md](references/behavior.md), with [craft.md](references/craft.md) for citations and prose
- design decisions and test seams — [design.md](references/design.md)
- contracts, call flow, and decomposition — [spec.md](references/spec.md)
- review comments and disagreement — [review-loop.md](references/review-loop.md)

## 5. Apply and re-run

Dry-run the typed result with `revy-plan check --result FILE` (or `-` for stdin), fix every problem it lists in one pass, then record it with `revy-plan apply`; it advances the plan atomically and prints `applied <kind> <key> to <plan path> → <state>` — check that it names the plan and packet you meant. `--work` takes the packet kind (e.g. `--work draft`) and may be omitted when one packet is ready. Cite files in the target repository as `path` and in a declared one as `alias:path` (the exploration packet's `repositories` lists each alias and its `cite_as`); a textual scope may be `.` or `alias:.` for a whole repository; keep each evidence item to one repository. Evidence may omit `target`: the CLI stamps the identity of the repository it cites — except anchorless evidence in a plan with several repositories, which must carry the identity it observed. If the checkout moved, run `revy-plan reopen exploration --recapture --reason TEXT` and re-apply the carried evidence (re-run any runtime evidence). An approved tool that failed goes in `tool_failures`, not silently dropped. Rerun `revy-plan status`, and run independent ready packets together. `revy-plan gate` succeeds only at `final` with no unresolved accepted findings (`--allow-accepted` waives the latter).

Edit, don't resend. A correction, and a `design_brief` or `draft` after a reopen, take `--patch` with id-addressed paths (`/candidate/content/risks/@risk-cache/statement` — `@<id>` matches an element's `id`, `subject`, `symbol`, or `lane`) rather than a full resent candidate. Cite a runtime transcript by the `{path, digest}` that `revy-plan transcript add FILE` prints; restarts and reopens then need no copied files.

Recover instead of restarting. When exploration missed something material after approval, `revy-plan reopen exploration --reason TEXT`: capabilities stay approved, and user decisions that come back unchanged keep their answers — when carried answers skip the user decision, tell the user in one line which earlier answers were reused and that `revise` changes them. When only the draft needs rework (a different MR split, a rewritten section), `revy-plan reopen draft --reason TEXT`. When the user or you must change the candidate under review or a final plan — acting on an advisory, on review comments, or on a user decision the user changed — `revy-plan revise --patch FILE --note TEXT [--addresses FINDING...]`, dry-run first with `--check`. While correcting, `--addresses` must name every must-fix finding; at a review escalation, every unresolved finding — and the user's ruling budget is not reset. Accepted findings of a final plan stay accepted unless the revision addresses them. The review budget otherwise restarts from the revision. Corrections may only reword a user decision, never change its answer; a revision that changes one records the change in `status` and the timeline — make it only with the user's approval and tell them.

Once a draft carries figures or code, `revy-plan render` and open the artifact — a plan a human cannot read is not delivered. Fix an over-long title, label, or note (see [presentation.md](references/presentation.md)) before the review waves, not at the finish.

## 6. Surface

Report, never absorb: capability limitations, capability amendments, every reopen and revision with its reason, decisions only the user can make, and any review escalation the CLI raises for a ruling. If status emits a blocked packet, report its remedy and wait for repair.

## 7. Finish at final

Finish when `revy-plan status` prints `final`. `revy-plan render` and `revy-plan serve` may run at any candidate-bearing state — `reviewing`, `correcting`, or `final` — and carry a draft watermark until the plan is final; render writes Markdown plus a standalone HTML artifact (authored plan first, derived figures in one folded appendix; `--brief` omits the appendix), and serve opens one loopback URL when reachable, otherwise deliver the standalone HTML artifact. Visual tools are optional. The renderer owns every drawing — [presentation.md](references/presentation.md) is which shape fits which content; never hand-write output.
