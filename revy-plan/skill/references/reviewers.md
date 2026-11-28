# Reviewers

A reviewer is a fresh subagent that did not draft the plan, given one lens. It returns findings only; you decide
what to do with them.

## Lenses

Dispatch a lens when the plan meets its condition. Size and shape reviewers to the risk:
- a **custom lens** for a risk no lens below covers
- a **quick check**: one question on one patch or section — *does the new criterion "empty input is rejected"
  contradict the slice that parses arguments?* — with the same brief, scoped to that question, on a lighter model when the question is
  mechanical

| Lens | Dispatch when the plan | Hunts for |
|---|---|---|
| `spec` | implements a request written by a person | a requirement readable two ways, an edge case nobody decided, a criterion that does not pin the request, a gap between the request and the plan |
| `consistency` | has more than one slice or decision | contradictions between goal, behavior, decisions, contracts, slices, and criteria; a decision no slice delivers; a slice that delivers nothing; an invariant in the Owner's view that no slice or test keeps |
| `impact` | changes existing code | a caller or consumer the evidence missed — re-enumerate references on the live tree for every changed symbol and count them; confirm or contest every `unaffected` and `n/a` lane with its own evidence |
| `failure` | adds I/O, mutation, shared state, async, retries, or a multi-step operation | a failure path without handling, a partial failure leaving state half-written, a missing compensation, a race, an ordering assumption |
| `data` | touches stored data, schemas, file or wire formats, flags, or the deploy | old code reading new data and the reverse, rollback, migration lock time, rerun after partial failure, deploy order |
| `tests` | has acceptance criteria | a criterion that cannot fail, a verify that passes with the work absent, a seam that misses a violating build, a new harness where the project's own would do; a test that protects nothing the rest of the set does not, a test added for coverage alone, a new test where an existing one could change, a test hard to read |
| `design` | makes decisions with real alternatives | a smaller change with most of the value, a strawmanned rejection, an unpriced one-way decision, types that admit illegal states, a duplicated rule |
| `clarity` | will be read by a person | an id or slug the reader must decode, jargon left undefined, a line the reader does not need, a paragraph that should be a list or diagram, a fact said twice |
| `security` | crosses a trust boundary — input parsing, auth, secrets, permissions | input trusted before parsing, an authorization gap, a secret exposed in logs or errors |

## Brief

Give each reviewer:
- the path of `plan.md`, and of `log.md` with the headings to read: `Problem`, `Capabilities`, `Evidence`
- its lens and what it hunts for, from the table
- its scope: the whole plan for the first review; for a re-review, the changed sections, the sections that
  depend on them, and the patch entry
- the repo root (and each other repository's root)
- an output path in `scratch/`

Tell it: verify claims against the live tree with the strongest capability listed, and run probes the way
evidence.md § Runtime evidence runs them. Each finding states:
- `severity`: `must-fix` (a bug, inconsistency, or unclarified requirement would ship) or `advisory`
- `where`: the plan section or slice
- `issue`: one sentence, standalone
- `evidence`: citation, marker, and what it showed
- `fix`: the concrete change to the plan

An empty review is a valid result; it states what was checked.

## Converging

Fix by default. An advisory finding is fixed or dropped with a one-line reason. A rejected must-fix finding
carries three lines in `Reviews`:
- `right:` the strongest case the finding is correct, citing the plan or code
- `wrong:` the evidence that it is not, from a marker stronger than `via:inference`
- `falsifier:` the observation that would prove the rejection wrong

When `right:` is the stronger case, fix it.

These go to the user with both sides quoted: two findings pulling opposite ways, a reviewer re-raising a
rejected finding, and any must-fix finding returned by the third review round.
