# unordered-iteration-reaches-output
langs: any

When: Set, hash map or unordered query result iterated into output, a key, a hash or signature, a "first" pick, or a paged list.
Ask: Is the order defined? Python `set` and `frozenset` of `str` change order per process (hash randomization); Go `map` order is randomized on purpose; Rust `HashMap` and `HashSet` are seeded per instance; SQL without `ORDER BY`, or with a non-unique one, returns any order, which changes with plans and between pages. Does that order reach something compared across runs or processes — a cache key, ETag, content hash, signature, golden file, `LIMIT 1`, a "first match wins" pick?
Not when: the consumer is order-insensitive (it re-sorts, or folds into a set or a sum).
Report as: `<collection>` at `<site>` is iterated in undefined order into `<output>`: `<keys differ across replicas / cache misses / the picked row changes / pages repeat rows>`; fix: sort on a unique key before use (`sorted()`, `BTreeMap`, `ORDER BY …, id`).
