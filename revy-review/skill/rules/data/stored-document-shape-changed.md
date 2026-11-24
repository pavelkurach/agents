# stored-document-shape-changed
langs: any

When: Shape written to a JSON column, document store, KV value, cache entry or queued message changed, or a new reader of one added.
Ask: Do readers handle every shape already stored or in flight — rows written earlier, messages sitting in queues and retry topics — including missing or renamed keys and old types, or do they index straight in? Is the value parsed into a typed model at the read boundary, with a version field or backfill? Does an older reader read-modify-write the value and drop keys it does not know?
Not when: the entry has a short TTL and old shapes expire before new readers ship.
Report as: `<reader>` assumes the new shape of `<document>`, but entries written earlier `<lack / differ in>` `<key>`: it `<crashes / misreads>` on old data that no test fixture holds; fix: parse with defaults for old shapes at the boundary, version the document, or backfill.
