# dependency-type-in-public-api
langs: rust

When: Third-party crate's type or trait placed in a `pub` signature, field, bound or trait impl of a library crate.
Ask: Is the dependency re-exported (`pub use dep;`) so callers can name the exact version? Is it pre-1.0 or often bumped — each of its semver-major releases becomes one of yours? Would a local newtype or trait keep it private?
Not when: one workspace pinning one version · stable 1.x vocabulary crates the ecosystem shares (`serde` traits).
Report as: `<pub item>` exposes `<dep>::<Type>`: callers on another `<dep>` version get "expected `T`, found `T`" or unsatisfied-trait errors, and upgrading `<dep>` becomes a breaking release; fix: re-export `<dep>`, or wrap the type locally.
