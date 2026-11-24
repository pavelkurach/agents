# guarded-reference-escapes-lock
langs: any

When: Value aliasing lock-guarded data (map, slice, list, pointer, iterator) used, returned or passed on after the unlock.
Ask: Was it copied under the lock, or does it share storage with the guarded field? Trace its uses after the unlock; search writers of the field that can run meanwhile, and check whether the callee takes the same lock.
Not when: the data is immutable once published · safe Rust, where borrows end with the guard.
Report as: `<value>` aliases `<field>` and is used after unlock at `<site>`: a data race (in Go, a fatal concurrent map access); fix: copy under the lock, or keep the work inside it.
