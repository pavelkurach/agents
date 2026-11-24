# revy-plan

An evidence-first implementation-planning skill for agent harnesses, driven by a small Rust CLI that owns the
whole workflow. `plan.json` is the sole editable artifact; the CLI is the deterministic driver — it owns
sequencing, derives the ready work packets, publishes their result schemas, runs the review loop, renders the
plan, and gates the handoff — while the model stays the planner and the judge. Exploration, drafting, and review
each run in a fresh, context-minimal subagent; the CLI decides what work is valid and what proves it done.

- `skill/` — the skill: `SKILL.md` (the orchestrator), the subagent references, `check_docs.py`.
- `cli/` — the `revy-plan` binary the skill runs (the planning library plus the flattened command tree).
- `docs/spec.md` — the spec: the run states, the work-packet contract, the review loop, and what each command enforces.

## Pipeline

`status` derives the ready frontier from one of eight run states; applying a typed work result advances the plan:

```
AwaitingCapabilityApproval → Exploring → AwaitingUserDecision → Drafting
  → Reviewing → Correcting → AwaitingReviewDecision → Final
```

The review states loop — `Reviewing`, `Correcting`, and `AwaitingReviewDecision` repeat per wave until a wave
clears — and `gate` succeeds only at `Final` with no unresolved accepted findings.

## Install

From this folder:
1. CLI: `cargo install --path cli` (puts `revy-plan` on PATH).
2. Skill: symlink the skill folder into your harness's skills directory, e.g.
   `ln -s "$PWD/skill" ~/.claude/skills/revy-plan`.

## Use

- `/revy-plan` — plan a non-trivial change evidence-first, before implementation.

The CLI, driving `plan.json` (located by `--plan`, or by `--target` and `--slug`):

```
revy-plan init      # create plan.json for a target repo from a capability report
revy-plan status    # print the ready work frontier for the plan's current state
revy-plan apply     # apply a checked work result, advancing the plan atomically
revy-plan render    # render the plan to Markdown and standalone HTML
revy-plan serve     # serve the rendered plan at one stable loopback URL
revy-plan gate      # succeed only at the final state
revy-plan schema    # print the generated JSON Schema, a section, or a packet's result schema
```

## License

MIT — see `LICENSE`.
