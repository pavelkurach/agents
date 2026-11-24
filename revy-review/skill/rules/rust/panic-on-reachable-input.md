# panic-on-reachable-input
langs: rust

When: `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, `v[i]`, `&s[a..b]`, integer division or `borrow_mut` added on a library or request path.
Ask: Can the value be `None` or `Err`, out of range, zero, or already borrowed on accepted input? Trace it to its source; `&str` slicing also panics off a char boundary. Does the crate set `panic = "abort"`, turning one bad request into a process exit?
Not when: tests, examples, build scripts, startup config · a local invariant named in `expect("…")`.
Report as: `<expr>` panics when `<condition>`, killing the request's thread or task `<or the process>` and poisoning held mutexes; fix: `Result` with `?`, `get(..)`, `checked_*`.
