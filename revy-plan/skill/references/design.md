# Design

Precise enough that the implementer makes no design decision.

## Decisions

One table row per decision. A decision needs at least two real alternatives; a single option is a constraint.

| Decision | Reversibility | Chosen | Cost paid — by whom | Rejected — and why it loses |
|---|---|---|---|---|
| Priority representation | one-way | `enum Priority` | stored tasks need a migration | *String*: no migration, admits invalid values at 6 call sites |

- Steelman each rejected option, then name the specific cost that kills it.
- `one-way`: wire formats, public ids, schema shape, auth boundaries — anything a consumer encodes against.
  Decide every one-way decision now; sort them first.
- The bar: types carry the invariants and parse at the boundary; the domain's words are the code's words; each
  rule lives in one place; reuse an existing seam, type, or module before adding one. A decision that misses the
  bar says why in its cost cell.
- Improving adjacent code is a scope change: it needs a stated non-goal or an explicit scope line.
- A decision that contradicts an ADR or documented convention says so in its row.

## Regime

**known-domain** (a variation on what the codebase already does — model types exhaustively) or **exploratory**
(the shape is uncertain — the first slice is a tracer bullet with minimal types). Known-domain is the default and
goes unstated; an exploratory plan says so in its Delivery section.

## Contracts

| Planned | Left to the implementer |
|---|---|
| every new or changed type — all fields, the full error taxonomy | internal helpers |
| signatures of public seams and changed APIs, before → after as code | private decomposition |
| the to-be call flow — happy path and every failure path | local control flow |

Type model, in the target language's real syntax, signatures only:
- the invariant each type closes, and where parsing happens
- every error variant (without typed errors: every message a caller can receive)
- where exhaustiveness is enforced
- what is load-bearing beyond the shape: derives, ownership, dispatch, `Send`, `#[non_exhaustive]`

Language forks that are decisions, not details:

| Language | Fork |
|---|---|
| Rust | where a bound is enforced — a `NonZero*` newtype deletes the failure *and* any criterion observing it |
| Go | no sum types: interface + switch vs `Kind` + nilable fields vs `Kind` + `(T, bool)` accessors; is the zero value valid |
| Python | exceptions vs `T \| Failure`; is the boundary library's model the domain type |
| TypeScript | the error carrier — thrown, `Result`, Effect — it changes every signature |

To-be flow: every failure path, with the compensating action wherever a failure follows a mutation. When the
change touches concurrent or transactional code, state locks, ordering, and what is atomic with what; otherwise
write `no concurrency surface`.

## Test seams

1. An existing seam first — this wins.
2. Then the highest one, closest to the public interface.
3. As few as possible.

Prove the seam, at the smallest cost that proves it:
- **Reaches the code** — a passing test that already exercises this code through the seam is the proof; cite it.
  Otherwise run a probe through the seam.
- **Guards the constraint** — for each constraint the plan exists to protect, show a test at the seam goes red
  on a build that violates it: a throwaway probe in `scratch/`, the smallest that shows it, never the full
  implementation. A seam that stays green against the violation does not guard it.

Record each run as evidence.

## Acceptance criteria

Criteria restate the behavior change and its edge cases as checks: every given / when / then example maps to a
criterion, or the plan says why it has none. Each criterion is a plain-English sentence with a `verify`:
- **falsifiable** — name the observation that would fail it
- **observable** — an outcome a caller or user sees; an internal mechanism only when it *is* the feature (a
  cache), saying why
- **not true by construction** — ask what implementation would fail it
- `verify` is a command that fails when the work is absent (it asserts on the runner's report of *that* test,
  since filter-based runners pass on a filter matching nothing), or a manual check naming what a person observes
- `verify` runs at implementation; checks you run while planning are evidence

## Tests

Be pragmatic: full coverage of the change with the fewest tests that give it. Coverage is every behavior
branch, failure path, and contract the change adds or alters. Every test **earns its place** by protecting
something the rest of the set does not: it catches a realistic bug, pins a contract a consumer relies on, or
locks a regression. A test that only moves a coverage number protects nothing.

- **Reuse first**: an existing test that already covers a criterion is cited as-is; an existing test whose
  assertion moves is changed in place; a new test is the last resort.
- **One behavior per test**, readable top to bottom — arrange, act, assert — with the expected value inline.
- **Variations of one behavior are rows** of one parametrized test (`rstest`, `pytest.mark.parametrize`, or the
  project's equivalent), so each body stays linear.
- **Real safety**: name what each test protects. A test asserting only that nothing crashed, or one
  restating the implementation, protects nothing — drop it or sharpen its assertion.
- **Each test sits at its cheapest seam**: unit for pure logic, the existing CLI or integration harness for
  wiring — one wiring test per path.

Each planned test carries a status:

| Status | Means |
|---|---|
| `new` | a test written for this change |
| `changed` | an existing test whose setup or assertion this change edits |
| `existing` | an existing test that already covers a criterion, unchanged — cited by its path and name |

## Slices

The plan ships as a graph of independently mergeable slices.

- **Vertical by default**: each slice makes a caller- or user-visible behavior work and carries its proof.
  A behaviorless foundation slice is justified only when several slices share it or a compile-compat change
  has nowhere else to go.
- **Smallest atomic**: split only where a reviewer could reject one slice and approve its neighbour; after each
  merge the tree is shippable.
- **Mergeability label**: `backward-compatible` · `feature-flag <name>` · `scaffolding (consumed by <slice>)` ·
  `breaking — <what changes for whom>`.
- **Data migrations** are their own slice, compatible both ways, ahead of their consumer: expand → migrate →
  contract, dropping the old form only after a zero-reader check.
- **Wide fan-out** of one contract change: expand → migrate caller groups → contract.

Each slice carries: a plain-English name saying what it delivers for a caller, files, `needs` (real ordering edges, including "its
verify needs that slice's harness"), its verify, its mergeability label, and the exact
signatures it consumes and provides (named, defined once in the plan's Changes section), and the binding
constraints it must keep.

Checks before the draft is done:
- every `consumes` is `provides`d by a slice reachable through `needs`
- every criterion is satisfied by exactly one slice
- every changed file appears in some slice's files; slices sharing a file are ordered or re-cut
