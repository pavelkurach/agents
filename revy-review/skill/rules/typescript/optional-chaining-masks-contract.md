# optional-chaining-masks-contract
langs: typescript

When: `?.`, `??` or `|| fallback` added on a value that was required, or a guarded call `fn?.()`, often beside a change to its producer.
Ask: Is absence legitimate here, or a broken invariant (failed load, renamed field, unwired dependency) that now passes silently? What does the code do with the `undefined` or the fallback — skip a write, render empty, allow access? Did the diff make the producer optional and push the checks onto readers?
Not when: the type and the producer document absence as a normal case.
Report as: `<expr>` at `<site>` turns a missing `<value>` into `<undefined / fallback>`: `<the save is skipped / the wrong default applies>` with no error; fix: fail loudly where the invariant breaks, or model absence in the type and handle it explicitly.
