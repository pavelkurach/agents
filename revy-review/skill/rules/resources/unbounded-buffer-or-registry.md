# unbounded-buffer-or-registry
langs: any

When: Unbounded queue or channel, insert-only map, cache or subscriber registry, or `@cache`/`lru_cache(maxsize=None)` over growing arguments added.
Ask: What bounds the key space or producer rate — user input, request ids, `self` on a cached method (which also keeps every instance alive)? Search for removal, including on error and disconnect paths. For a bounded structure, does a full one block, drop or reject, and do producers expect that?
Not when: the keys form a small closed set · the structure dies with the request.
Report as: `<structure>` grows with `<driver>` and is never pruned: memory rises until OOM; fix: a capacity with an explicit full policy, or eviction or TTL.
