# how-it-works

The whole flow, start to finish — from the user's first action to the last observable outcome, seen from the
user's side and the code's.

## Enumerate

| Enumerate | Scope |
|---|---|
| Entry points | every way the flow starts — UI action, API call, CLI, job, webhook |
| Actors | the user, each process, service, and component, each external system |
| Containers | each separately run or stored piece the flow crosses, and the channel between each pair — HTTP, queue, file, CLI call |
| Stages | every hop from trigger to final outcome; no hop left implied |
| Branches | success, each failure, each alternative path (*SSO vs password*) |
| State | what is stored, where, and its lifecycle — created, read, refreshed, expired, revoked |
| Knobs | config, flags, env, and settings that change the flow |
| Tests | what covers the flow today |

## Body

1. **User journey** — a journey or flow diagram of what the user sees and does at each step, failures included.
2. **Containers** — a C4 container diagram of what the flow crosses, each edge naming its channel; merged into
   the Map when the subject runs in one container.
3. **End to end** — a sequence diagram, every actor, trigger to final outcome; each failure and
   alternative as an `alt` block.
4. **Stages** — a table: stage, what happens, where in code, input → output.
5. **State** — a state diagram for each stateful thing, with where each transition happens in code. Draw it
   from the transitions the code performs — walked with code intelligence or a probe — never from names or docs.
6. **Failures** — a table: trigger, where it is caught, what the user sees.
7. **Knobs** — a table: setting, default, effect on the flow.
8. **Tests** — a table: what each test covers; the stages and branches where no test was found, with the search
   that came back empty — a search result, not a coverage measurement.
