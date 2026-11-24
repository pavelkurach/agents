# catch-all-over-closed-set
langs: any

When: `match`/`switch`, `if/elif` chain or handler map over an enum, union, status set or `kind` discriminant added or edited.
Ask: Which known members reach the default or fall through today, and is that right for each? When a member is added later, what fails: Python `assert_never`, a Rust `match` with no `_` arm, in TS only a `default` with `assertNever(x)` or `x satisfies never`, or a return type excluding `undefined` (void branches and an `else` absorb new members), or a test that every member has a handler? Is a handler map keyed by raw strings instead of the enum?
Not when: values come from an external producer that may send unknown ones (the default then logs them) · a Rust `match` with no wildcard (rustc already fails the build).
Report as: `<site>` sends every unlisted `<set>` member to `<default>`: the next member added is `<mishandled / dropped>` with no error; fix: list each member and end with an exhaustiveness check (`never` in TS, `assert_never`), or key the map by the enum (`Record<Union, …>`) and test its coverage.
