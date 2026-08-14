# Mandatory Agent Rules

**Mandatory.** Every rule below is a completion requirement whenever its subject exists. A skill may add process or specialize application; it cannot waive a rule. Before completion, account for every rule as satisfied or inapplicable because its subject is absent.

## Engineering Practice

- **Evidence first.** Read relevant code, tests, and config. Verify APIs, symbols, and behavior from source; distinguish facts from hypotheses.
- **Scope lock.** Make only the requested or approved behavior delta. Preserve unrelated work. Surface material assumptions, edge cases, conflicts, and open questions; stop for decisions that materially change the result.
- **Fail loud.** Handle boundaries explicitly. Use precise, actionable errors; preserve consistency and idempotence across multi-step work.
- **YAGNI.** Prefer the smallest clear solution, existing patterns, stdlib, and one source of truth. Separate refactors from behavior changes; remove dead code.
- **Comments explain why.** Keep only non-inferable constraints, workarounds, and contracts. Express what the code does through names and structure.
- **Code intelligence first.** Resolve definitions, callers, references, implementations, and call hierarchy semantically with LSP or code graphs. Use text search for text.
- **Terse.** Communicate as change · rationale; alternatives · trade-offs; issue · cause · recommendation.
- **Priority.** Correct → clear → safe → clean → fast. Optimize measured bottlenecks.

## Type-Driven Design

- **Illegal states.** Make illegal states unrepresentable. Model domain primitives, alternatives, and transitions with dedicated types, sum types, private constructors, and exhaustive matching.
- **Parse, don't validate.** Convert raw input into proven domain types at boundaries, then trust those types internally.
- **Types over checks.** Prefer type-level prevention to runtime checks. Use strict, explicit types throughout; `Any` and untyped containers are invalid.

## Functional First

- **Functional core.** Build behavior from deterministic pure functions and composition.
- **Immutable data.** Prefer values and transformations to hidden mutable state.
- **Imperative shell.** Isolate mutation, I/O, time, and external systems at the edges.
- **State earns its place.** Use classes or mutable state only when domain identity, lifecycle, or concurrency requires them.

## Tests Are Specifications

- **Behavior, not implementation.** Test observable contracts rather than internal structure.
- **Edges.** Cover happy paths, failures, transitions, and relevant null, numeric, text, collection, time, concurrency, and I/O boundaries.
- **AAA.** One behavior per test; Arrange–Act–Assert; scenario-and-outcome names; inline expected values; linear bodies without control flow.
- **Real core.** Exercise domain logic directly. Fake only external systems, I/O, clocks, and nondeterminism; integration tests use real internal dependencies.
- **Isolation.** Keep tests minimal, order-independent, and parallel-safe.
- **Coverage.** Cover behavior branches, not line counts; prioritize business-critical code.
- **Regression lock.** Reproduce every bug with a failing test before fixing it; retain the passing test. Treat flakes as bugs.

## Implementation Completion

- **Green.** A code change is complete only when the approved behavior works, relevant failures are covered, relevant tests pass, and the diff survives scope and correctness review.

## Subagents

- **Checkpointing.** For every subagent, define a task-local checkpoint that records completed work, remaining work, and restart instructions. Require the subagent to update it after each completed unit of work so another agent can resume without repeating completed work.

## Superpowers

- **Explicit approval.** Ask before using Superpowers skills, and use them only for large tasks.
