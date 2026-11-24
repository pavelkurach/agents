# Type model

Real syntax of the target language, signatures only, no bodies. This file is what the **plan** must
state, and which choices are decision rows.

## Every type model states

- **The invariant each type closes.** A newtype closing none is an alias with ceremony — justify it
  (preventing argument transposition) or drop it.
- **Where parsing happens.** That boundary is the whole of *parse, don't validate*; inside it the
  type is trusted.
- **Error variants, exhaustively.** `Result<T, StoreError>` without its variants specifies nothing
  about failure. Where the codebase has no typed errors, it is the set of messages a caller can
  receive, enumerated the same way.
- **Where exhaustiveness is enforced**, and by what — `assert_never`, `const _: never`, a match arm.
- **What is load-bearing beyond the shape** — derives a later call needs (`Copy` for `sort_by_key`,
  variant order for `Ord`), ownership (`&` / `&mut` / owned), dispatch (static vs `dyn`), `Send`
  bounds, `#[non_exhaustive]`, `context.Context` position.

## What the decision owes, and what waits for the type model

A design decision has no signatures, so it cannot state a taxonomy. What it states is the **fork**:
whether an operation can fail at all, and **where the validation lives** — at a constructor, at a
setter, in the type. That fork carries a price, and it decides whether an acceptance criterion is
observable: a value validated by its type has no failure anyone can test, and the criterion demanding
one has to be retired rather than quietly left unmet.

The exhaustive variant list belongs to the type model, and only reaches back into a decision when a
*variant* is the fork — when one of them is the thing a caller must handle differently.

## Forks, by language

These are decision rows, not type-model details.

| Language | Fork | Trap the plan must not walk into |
|---|---|---|
| Go | **No sum types.** Interface + type switch (open set, no exhaustiveness) vs `Kind` field + nilable fields (illegal states representable) vs `Kind` + `(T, bool)` accessors (verbose, misuse visible). Also: is the zero value valid, and what stops it escaping? | The invariant comment on a struct *is* the enforcement — there is nothing else. Interfaces belong at the consumer |
| Python | Exceptions vs `T \| Failure`, per operation. And whether the boundary library's model type (pydantic / attrs / cattrs) **is** the domain type or converts into one. | A union with no `Literal` discriminant cannot be narrowed; a frozen dataclass validating nothing is a named tuple |
| TypeScript | The error carrier — thrown, `Result`, `neverthrow`, Effect. It changes every signature. And which cases still legitimately `throw`. | A brand is worth exactly the rule that its constructor is the only way to make one — state that rule |
| Rust | **Where a bound is enforced**: a constrained newtype or `NonZero*` moves the check to construction and deletes the failure — and with it any criterion that was going to observe it — while a checked `usize` keeps the error and the test. Also typestate, when misuse is plausible; not for a two-state object with one call site. | Reaching for `NonZero*` because it is tidier, without noticing an acceptance criterion just became unobservable. The tell you need an `enum` is a doc comment saying "if `a` is `Some` then `b` must be `None`" |
