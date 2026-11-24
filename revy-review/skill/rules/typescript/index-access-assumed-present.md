# index-access-assumed-present
langs: typescript

When: `arr[i]`, `arr[0]`, `record[key]` or an index-signature property read and used without a check.
Ask: Is `noUncheckedIndexedAccess` set in tsconfig (`strict` leaves it off)? Without it indexing yields `T`, not `T | undefined`, so an empty array, an out-of-range index or a missing key compiles and fails at runtime. Can the collection be empty, or the key come from input?
Not when: the flag is on · the index is bounded by the same array's length · a `Record` keyed by a closed union.
Report as: `<expr>` at `<site>` is typed `<T>` but is `undefined` when `<empty / key missing>`: `<crash / undefined written out>`; fix: check the element (`.at()`, `Map.get` and `find` return `T | undefined`), and enable `noUncheckedIndexedAccess`.
