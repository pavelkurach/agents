---
name: revy-plan
description: Use for `/revy-plan`, technical specs, design requests, or implementation-plan requests for non-trivial code changes before implementation.
---

# revy-plan

Drive the Rust `revy-plan` CLI. The CLI owns sequencing, work packets, result schemas, completion, subjects, blockers, and rendering. Never restate a schema or mandate it emits — run `revy-plan schema` and `revy-plan status` to read them.

## 1. Locate capabilities

Find, and name by purpose not product: the `revy-plan` binary on `PATH`; the external plan home, where plan.json lives outside the target checkout; filesystem and shell; an isolated review context; the evidence capabilities available now — code-intelligence (LSP, call hierarchy, references), runtime execution, structured and text search; and a delivery path, a reachable loopback URL or a file artifact. No tool product is mandatory. Reading source and grep are the universal floor every method falls back to.

## 2. Init or resume

Inventory the capabilities, write them to `capabilities.json`, and `revy-plan init --capabilities capabilities.json` creates plan.json in the capability-approval state; resume by reading `revy-plan status`. Execute the capability-approval packet it emits.

## 3. Present, then wait

Present the capability report and wait for the user's exact approved decision. Capability approval is required before any substantive exploration. Accept only the exact reply the packet emits.

## 4. Execute each ready packet

```
revy-plan status
```

For every ready packet, treat its emitted mandate, subject, schema, and completion criteria as the whole instruction. Execute it with the best approved evidence method for its uncertainty; where that method is unavailable or fails, fall back to reading source and grep, and record the weaker evidence honestly. Proactively recommend a higher-value capability when its benefit, cost, side effects, and one limitation are worth stating — recommend, never require. Reviews are derived from the candidate — an integrated review always, plus a specialist for each impact shape that triggers one — not a fixed roster; each runs independently in a fresh, context-minimal subagent — never a full-history fork — briefed only with the candidate artifact path, the review packet, the repo root, a checkpoint path, and findings-only instructions; the whole derived set dispatches concurrently. Reviewers return findings only — they never resolve, triage, or edit the plan; the integrated review owes three things: findings, a verdict for every `not_considered` lane, and a consumer check for every changed symbol. A correction is a `--patch` plus a `triage` that names every material finding.

**Orchestrate; do not absorb.** The main session runs the CLI and the user's turns; each packet's tool-heavy work runs in a fresh, context-minimal subagent — **never a full-history fork**. Exploration (code-intelligence, search, reading source) and drafting (reasoning over that evidence) are the tool-heavy packets: dispatch each to its own subagent. The subagent runs the whole loop and returns only the packet's typed result and its symbol-citations — never raw tool output; hand it the packet's mandate, subject, schema, any input artifact **by path** (never paste evidence), and a checkpoint path it updates so another agent can resume. One subagent per packet, not per query, so a code-intelligence server onboards once. The main session then holds only what §6 reserves for it — statuses, artifact paths, and the decisions the user must make; raw LSP and grep dumps degrade planning judgement and re-cost every turn they sit in context. The
plan is the memory: `plan.json` and `revy-plan status` outrank your recollection — after any context
loss, re-read status before the next decision.

Match the packet to its judgement:

- evidence, bug reproduction, and pushback — [behavior.md](references/behavior.md), with [craft.md](references/craft.md) for citations and prose
- design decisions and test seams — [design.md](references/design.md)
- contracts, call flow, and decomposition — [spec.md](references/spec.md)
- review comments and disagreement — [review-loop.md](references/review-loop.md)

## 5. Apply and re-run

Record the typed result with `revy-plan apply`; it advances the plan atomically. Rerun `revy-plan status`, and run independent ready packets together. `revy-plan gate` succeeds only at `final` with no unresolved accepted findings (`--allow-accepted` waives the latter).

Once a draft carries figures or code, `revy-plan render` and open the artifact — a plan a human cannot read is not delivered. Fix an over-long title, label, or note (see [presentation.md](references/presentation.md)) before the review waves, not at the finish.

## 6. Surface

Report, never absorb: capability limitations, capability amendments, decisions only the user can make, and any review escalation the CLI raises for a ruling. If status emits a blocked packet, report its remedy and wait for repair.

## 7. Finish at final

Finish when `revy-plan status` prints `final`. `revy-plan render` and `revy-plan serve` may run at any candidate-bearing state — `reviewing`, `correcting`, or `final` — and carry a draft watermark until the plan is final; render writes Markdown plus a standalone HTML artifact, and serve opens one loopback URL when reachable, otherwise deliver the standalone HTML artifact. Visual tools are optional. The renderer owns every drawing — [presentation.md](references/presentation.md) is which shape fits which content; never hand-write output.
