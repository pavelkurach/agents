# array-sort-default-and-mutation
langs: typescript

When: `.sort()` without a comparator, or `.sort()`/`.reverse()`/`.splice()` on an array the code did not create (parameter, prop, state, cached value).
Ask: Default `sort` compares as strings (`[10, 9, 1]` → `[1, 10, 9]`): does the array hold numbers, or need locale order? `sort` and `reverse` reorder in place and return the same array: who else holds it (caller, React state, a memoized value)? `toSorted` and `toReversed` (ES2023, Node 20+) return copies.
Report as: `<call>` at `<site>` `<sorts numbers as strings / reorders the caller's array>`: `<wrong order / caller and render state change underneath>`; fix: a comparator (`(a, b) => a - b`, `localeCompare`) on `toSorted` or a copy.
