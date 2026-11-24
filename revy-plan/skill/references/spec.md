# Contracts and flow

Precise enough that the implementing agent makes no design decisions.

## Contracts only

| Planned | Left to implementation |
|---|---|
| Every new or changed **type**, exhaustively — all fields, complete error taxonomy | Internal helper signatures |
| Signatures of **public seams** and changed public APIs | Private decomposition |
| **Complete call flow** — happy path and every failure path | Local control flow |

Exhaustive on types is not negotiable **in the known-domain regime**: one missing field is one
permitted illegal state. In `exploratory` it is scoped per slice — see below.

## Regime

State which, in one line. **known-domain** — a variation on something the codebase already does.
**exploratory** — the right shape is genuinely uncertain; the first slice is a tracer bullet with
minimal types and the model completes in a later slice.

A types-and-taxonomy-only first slice ships a one-way format change delivering nothing a user can see,
opening a window where the rollback hazard is live and the feature is not. An exhaustive type model
approved before any consumer exists is the **outrun-your-headlights failure**: committing to shapes
before implementation has taught you anything. Prefer folding a compatibility-safe type change into the
first behavior slice whenever that slice stays green.

## Type model

Per [type-model.md](type-model.md). Real syntax, signatures only, no bodies. Entities, fields and keys
carry their keys explicitly, so a field referencing another entity is not mistaken for a plain column.

## To-be call stacks

Signatures now, no provenance markers — these frames don't exist yet.

```
run_add(args) -> ExitCode
  -> Title::parse(&str) -> Result<Title, InvalidTitle>
       Err -> exit 2, "title must not be empty"
  -> Store::add(Title, Priority) -> Result<TaskId, StoreError>
       Io -> exit 1
  -> Store::flush() -> Result<(), StoreError>
       Err -> Store::remove(id)      // compensation — state already mutated
            -> exit 1
```

Happy path **and every failure path**, with the compensating action shown wherever a failure follows a
mutation. A flow that only draws success is not a design. State concurrency and ordering — locks,
transaction boundaries, what is atomic with what — when the change touches concurrent or transactional
code. Otherwise say `no concurrency surface`.

## Seams and file structure

Which seams, at what height, is a decision. Here: their production and test adapters. One adapter is a
hypothetical seam; two is a real one.

Then map every file created or modified and what each is responsible for — **before** slicing the
graph. File boundaries *are* the decomposition's contention edges. One responsibility per file; things
that change together live together. Follow the existing layout; splitting a file that has grown
unwieldy is a decision, not a silent choice.

## Coverage

Every acceptance criterion appears in the test plan **or** the QA plan, in exactly one; is claimed by
**exactly one** slice; and every id a slice claims resolves to a real criterion. Exactly one, not at
least one.

**Every constraint has a named guard, and each guard states the build that would violate it.** Say
which guards you executed and which you only argued. Criteria-only coverage ticks green on a spec whose
most-argued constraint has no guard at all, and on a test that cannot fail — if the function under test
never opens a file, "the store bytes are unchanged" is a tautology. **Prior art** cites an existing
test of the same shape, opened and confirmed to contain the pattern; `none — new shape` is the honest
answer more often than it looks.

**`verify` is planned; nothing runs at planning time.** Each acceptance criterion carries a `verify` —
a command (`program`, `args`, `cwd`, `expected_status`) that will run at implementation, or a `manual`
procedure naming what a person must check — never executed while the plan is being written. There is no
pass/fail outcome to record here and nothing to stub. A check the planner *did* run against existing
code — a formatter, a lint, the pre-existing suite, a bug reproduction — is not a `verify` outcome at
all: it is `Evidence::Runtime` on the observation it produced, carried in the exploration, not in the
delivery plan.

Then decompose into an independently mergeable graph — [mr-dag.md](mr-dag.md).
