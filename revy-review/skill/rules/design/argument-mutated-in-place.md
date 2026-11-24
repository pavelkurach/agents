# argument-mutated-in-place
langs: any

When: Function modifies a list, dict, object or struct its caller passed in, or an out-parameter added.
Ask: Read the callers: do they use the object afterwards, or pass something shared — a value returned by a cache or memoized function, a module constant or default, a config object, the list they are iterating? Do the name and signature say it mutates (in Rust, is mutation hidden behind `&self` with `RefCell` or `Cell`)?
Not when: a method mutating its own receiver · a builder or accumulator passed for that purpose and named so.
Report as: `<fn>` mutates the caller's `<arg>`, which `<caller uses afterwards / every later reader shares>`: `<it sees a changed value / the cached copy is corrupted>`; fix: return a new value, or make it a method on the owner whose name says it mutates.
