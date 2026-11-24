# Decomposition graph

An independently mergeable graph of slices, the standalone implementation contract. **A graph, not a
sequence.** Optimise for shortest wall-clock at the harness's available parallelism, not maximum width.

**Default to vertical delivery.** Every ordinary slice makes an observable caller or user behavior work
and carries its proof. A behaviorless root is justified when no CI-green behavior slice can absorb a
compile-compatibility change, when implementers would contend on the same foundation files, or when one
foundation is shared by several downstream slices. Fold a compatibility-safe type or schema change into
the first behavior slice when it serves that one slice; when it serves many, extract it so each vertical
slice stays small. A foundation slice is never speculative: build only what a planned downstream slice
consumes.

**Wide mechanical fan-out is the exception.** When one contract change reaches too many callers for a
reviewable green slice, use expand → migrate → contract: a behaviorless compatibility root adds the new
form beside the old; migrate bounded, dependency-ordered caller groups while both compile; remove the
old form only after a deterministic zero-consumer check. Each migration group is still a vertical
behavior or ownership slice, never an arbitrary file batch.

## What makes a slice atomic

Independently mergeable. Every slice declares how:

- `backward-compatible` — old and new paths both work
- `feature-flag <name>` — new path exists but is off
- `scaffolding` — unused code, no behavior change; names the slice that consumes it
- `breaking — <what changes for whom>` — none of the above is true

**Slicing does not make a label true.** A stored-format change, or a flag value that used to be
accepted and now errors, is neither backward-compatible nor scaffolding, and inventing a feature flag
for a 150-line CLI is worse than saying what breaks.

**A schema or data migration is its own slice, compatible both ways.** Never fold a stored-format
change into the code that depends on it: ship the migration alone, ahead of its consumer. Across the
rollout window it must be backward- and forward-compatible — old code reads what the new schema writes,
new code reads what the old schema left — so deploy order and rollback are both safe. Expand → migrate →
contract is the shape: add the new form, backfill, cut readers over, drop the old form only after a
zero-reader check.

**Cut to the smallest atomic slice — and no smaller.** The floor is the smallest change that still
clears all three tests below, and it cuts both ways: bundling deltas that could each be reviewed,
verified, and released alone is too coarse; fragmenting one behavior so an intermediate merge leaves the
tree unshippable is too fine. Several independent units of the same shape — commands, endpoints,
migrations — are one slice each plus any shared groundwork, never one bundle. Smallest is set by the
tests, not a line count, and not by width: a slice that only fills a wave is the wave-filler smell.

Three tests, all must pass. **Independence** — split only where a reviewer could meaningfully reject
one slice while approving its neighbour; setup, config and docs fold into the slice whose deliverable
needs them. **Reviewability** — each slice is independently understandable and testable: its `verify`
(see Per slice) fails when the slice's work is absent. **Releasability** — after it merges the tree is
shippable on its own: behavior complete, backward-compatible, or off behind a flag, never half-exposed.
There is no size field: a self-estimate is a guess, not a measurement.

## Two edge kinds

| Edge | Means | Breakable? |
|---|---|---|
| `needs` | B calls a type or API A introduces, **or B's `verify` cannot run without A** | No — real ordering |
| `touches` | A and B edit the same file | **Yes** — usually removable by re-slicing along file boundaries |

A test harness every other slice verifies against is a `needs` edge even though nothing imports it: a
verify-only root can merge early, a compile root cannot. `needs` is authored; `touches` is derived from
the pairwise intersection of the slices' files. The limiter on parallel work is rarely logical
dependency — it is file contention. Fill each wave to the available parallelism and no wider, minimise
critical-path depth, then minimise `touches` edges within a wave. A wave wider than that is a queue
drawn sideways.

## Per slice

Each slice carries, so an implementer can take one without reading the rest of the plan. Some are
**typed fields the CLI validates**; the rest are **prose you place in `standalone_behavior` or a
`Section`** — the CLI carries no field for them, so authoring one as structured data is a schema error at
`apply`.

Typed fields — `title`, `files`, `needs`, `satisfies`, `standalone_behavior`, `verify`:

- **`title`** — what changes for a user or caller, not the mechanism.
- **`files`** — every file it touches. Every changed symbol's `path` must land in some slice's `files`,
  and the `touches` contention edges are derived from the pairwise intersection of these lists.
- **`needs`** — the real ordering edges (below).
- **`satisfies`** — which acceptance criteria this makes true, by slug. A slice satisfying nothing is
  scaffolding and must say so.
- **`verify`** — a **command** (`program`, `args`, `cwd`, and the `expected_status` it must exit with)
  that **fails when this slice's work is absent**, or omitted when the slice has none. Naming a test the
  slice creates is not enough: name-filtering runners exit 0 on a filter matching nothing, so
  `cargo test -- typo` reports `0 passed; 6 filtered out` and succeeds. The command has to assert on the
  runner's own report of *that test*:

  ```
  cargo test -p store --lib -- --exact lease::expiry | grep -q '^test lease::expiry \.\.\. ok$'
  pytest -q tests/test_lease.py::test_expiry | grep -qE '^1 passed'
  ```

  A filter you have watched fail proves the test can fail, not that its absence is caught. Where the
  runner asserts natively (`pytest --strict-markers`, `--fail-on-empty`), use that. **A slice whose
  deliverable is a failing test inverts the check** — its `verify` greps for the failure, and says so.

Prose, in `standalone_behavior` or a `Section` — never a typed field:

- **`consumes` / `provides`** — **exact signatures, not descriptions.** An implementer sees only their
  own slice; this is the only way they learn what a neighbour named things. For a service these are
  endpoint contracts — `POST /v1/x — 409 {...}`.
- **`constraints`** — the binding constraints, copied verbatim; an implementer sees one slice and none
  of the surrounding document.

Ids are a short kebab-case slug — a number implies a sequence the graph does not have.

**Invariants — read by hand, since `consumes`/`provides`/`constraints` are prose the CLI does not
parse.** Every `consumes` symbol appears in the `provides` of a slice reachable through `needs` — a
`consumes` with no `needs` path is a slice that cannot compile. Every `provides` resolves to a
declaration in the spec — the type model, an endpoint, the observability section — and a service slice
provides HTTP statuses, a metric name, a script and its output, not the type model alone. Every
`satisfies` id resolves to a criterion. These check that what you declared is consistent, never that it
is what the slice does; read each slice's lists against the flow it implements.

## Handoff

Hand each slice to the implementation agent with [review-loop.md](review-loop.md), preserving the
evidence-backed challenge bar in code review. Every plan records deviations: each slice requires the
implementer to record what the plan said, what was done, why it differed, and the files actually
touched. A touched file absent from a slice's `files` is a missed blast-radius consumer and remains
visible as the plan's accuracy feedback.

**A single-slice graph is a smell only when a decomposable change was lumped into one slice** — several
deliverables a reviewer could accept or reject separately, collapsed into one node. A genuinely small,
coherent change is legitimately a single slice — the same case as folding a compatibility-safe change
into the one behavior slice it serves; the smell is the missing decomposition, never the count itself.
The opposite over-slicing error: a slice that exists because parallelism is available, rather than
because a reviewer could reject it alone, is a wave-filler.
