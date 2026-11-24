# invariant-split-across-locks
langs: any

When: Fields that must agree (count and items, index and map, two caches) updated under different locks or in separate critical sections.
Ask: Which readers assume they agree, and can one run between the two updates? Read each reader's lock scope.
Report as: `<A>` and `<B>` must agree but update separately: a reader between the updates sees `<torn state>`; fix: one lock over both, or swap in one immutable snapshot.
