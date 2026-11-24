# unsafe-without-safety-argument
langs: rust

When: `unsafe` block or fn, `unsafe impl Send`/`Sync`, or a raw FFI call outside a safe wrapper added or edited.
Ask: Does a `// SAFETY:` comment name every invariant — non-null, aligned, initialized, unaliased, lifetime, thread-safety — and does the surrounding code guarantee each? Read the callers of an `unsafe fn` and the fields of a `Send`/`Sync` type. Would a safe API do, and is the block minimal and wrapped in a safe interface?
Report as: `unsafe` at `<site>` relies on `<invariant>`, which `<is unstated / fails when …>`: undefined behaviour; fix: `<safe alternative>`, or shrink, encapsulate and justify it.
