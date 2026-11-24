# Design

The approach as decisions with prices. The last cheap place to change direction before the contracts
become expensive to throw away.

Read [type-model.md](type-model.md) first — several type choices are decisions, not details.

## The bar a design is held to

Not preferences. A decision that breaks one of these owes the reason in its `Cost paid` cell:

- **Types carry the invariants** — illegal states unrepresentable, parsing at the boundary.
- **The domain's words are the code's words**, wherever the codebase has a domain worth naming.
- **A rule lives in one place.** Duplicated *decisions* are the expensive kind; name where each rule
  lives.
- **Reuse before addition** — say what a decision reuses, an existing seam, type or module, before what
  it introduces.

None of these licenses restructuring code the change does not touch. A design that improves something
adjacent has changed scope, which needs a non-goal or a behavior-delta row.

## It is a table, not an essay

| # | Decision | Reversibility | Chosen | Cost paid | Rejected — and why it loses |
|---|---|---|---|---|---|
| 1 | Priority representation | **one-way** | `enum Priority` | migration for stored tasks | *String*: no migration, but admits invalid values at 6 call sites |
| 2 | Sort order | reversible | explicit `--sort` flag | one more flag | *always sort*: silently changes existing output |

- **Steelman every rejection.** State its strongest form, then the specific cost that kills it.
- **`Cost paid` names who pays** — a user, a caller, or a future change, never the diff. "One more
  flag" is a shrug: the real price of an opt-in flag is that the stated problem persists for everyone
  who does not type it.
- **No decision without an alternative** — a single-option item is a constraint, not a decision.
- A decision contradicting an ADR or a documented preference says so in its row.
- **The `Rejected` column ships in the plan** — carry it into the answered decision's `ruled_out`
  (each entry an `{ alternative, reason }`), on planner- and user-owned decisions alike. It renders as
  a "Considered and rejected" sub-list under the decision, so the negations that make the design
  legible ("there is no two-phase register/ack — the actor already serializes every pop") are visible
  to the reader, not lost with the brainstorm. A settled decision with a real alternative that omits
  `ruled_out` has dropped its most load-bearing sentence.

## Reversibility is the column that matters

`one-way` — expensive or impossible to undo once data or callers depend on it: wire format, public
IDs, schema shape, auth and ownership boundaries, anything a consumer will encode against.

**Every one-way decision is decided now**, even when the feature it serves ships much later — getting
it wrong forecloses the design. Reversible ones may defer, and the smallest first cut is the one that
proves the approach without foreclosing it. Sort one-way rows first.

## Test seams

Decided with the design, not with the test plan — which seam you test at shapes the design.

1. **Prefer an existing seam** — this dominates. A new one is a decision that must earn itself.
2. Then the highest seam possible, closest to the public interface.
3. Fewest possible. One is the ideal.

Rules 1 and 2 point opposite ways whenever the existing seam is mid-height, the common case. **Rule 1
wins** — but it needs a candidate: where no existing seam can observe the thing (syscall counts,
residency, wall-clock), a new one is the only option.

**Execute the seam before committing to it.** An existing swappable attribute is not a seam until
injection is shown to work: class attributes do not cross a process-spawn boundary, monkeypatching does
not survive a re-import.

**Then execute a build that violates each constraint the seam is meant to guard.** A build that sorts
unconditionally, or loads the store before parsing its arguments, can stay green through the whole
suite while breaking the constraint the plan exists to protect. **A seam that stays green against a
violating build is not a seam for that constraint.** Where an approved decision requires carrying that
blind spot — a criterion written loose enough that no policy is presupposed cannot catch the wrong
policy — say so in the decision's `Cost paid` cell and surface it.

## Annotated call stack

The as-is stack, marked. No signatures — those come with the type model. This is the design as a diff:

```
handle_list                    src/main.rs::handle_list
  ~ Store::all                 src/lib.rs::Store::all      now takes SortKey
  + Priority::cmp                                          new
  ~ print_table                src/main.rs::print_table    priority column
```

`+` added · `-` removed · `~` changed · `->` untouched.

**Where designing shows the behavior work wrong, fix it and carry on.** It is the same plan and nothing
has been approved, so it is an edit, not an incident.
