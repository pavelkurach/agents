# task

The context a reader needs to solve the task themselves: the flows, the code, and the constraints every solution
stands on. The reader takes this into planning, where a planner asks them the questions whose answers shape the
design; the explanation readies them to answer each one. For a bug, the reproduction and the flow it runs
through come first.
The Answer names what the task turns on: the decisions it forces and what each depends on.

## Enumerate

| Enumerate | Scope |
|---|---|
| The ask | the task as given, its source quoted — issue, ticket, message |
| Behaviour today | run, never only read: a bug reproduced; for a feature, the closest existing behaviour run with the project's own command, its output and help text recorded (evidence.md § Runtime evidence) |
| Root cause | for a bug, after the reproduction: a deep root cause analysis per evidence.md § Root cause |
| Flows | every flow the asked behaviour passes through — its entry points, actors, stages, and branches per how-it-works.md § Enumerate |
| Code map | the files and symbols in those flows and the role each plays |
| Constraints | invariants, contracts, consumers one hop out from those symbols, every reader of what they produce, config, compatibility promises |
| Precedents | how the codebase already solves similar problems — the patterns a solution would follow or break |
| Tests | covering the area today, how to run them, where a new one would sit |
| Choice points | every decision a solution must make — behaviour the ask leaves open, constraints that pull against each other, precedents that disagree |

## Body

1. **The ask** — quoted, then restated plainly; its acceptance as given / when / then rows when the source has
   any.
2. **Today vs asked** — side by side. For a bug: observed vs expected, the reproduction command and its output.
   For a feature: the closest existing behaviour vs the asked one.
3. **Root cause** — for a bug: a flowchart from trigger through propagation to the root cause, each hop cited,
   the root cause marked `failure`; then a table of what else the same cause breaks.
4. **Flows** — per flow, a sequence diagram: every actor, trigger to final outcome, each failure as
   an `alt` block, the part the task touches in a `focus` block.
5. **Code map** — a table: symbol, role, flows it sits in.
6. **Constraints** — what any solution must keep true, each with its evidence id.
7. **Precedents** — a table: pattern, where it is used, what it implies for this task.
8. **Tests** — a table: test, what it covers, how to run it.
9. **Choice points** — a table: the question a planner would ask, in one line; what it affects; the options the
   evidence shows, each with its ids; who can settle it. Options stay unranked — the route is the reader's.
