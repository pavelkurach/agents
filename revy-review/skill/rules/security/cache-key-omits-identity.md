# cache-key-omits-identity
langs: any

When: Response or computed value cached (memoize, Redis, CDN or HTTP cache headers, semantic cache), or the inputs of a cached function changed.
Ask: List every input the value depends on — user, tenant, roles, locale, flags — reading the body, since identity often comes from request context rather than arguments, and compare the list with the key. Does a personalized HTTP response send `Cache-Control: private` or `no-store`, or `Vary` on the auth header? Is a similarity-cache hit restricted to the same identity scope?
Not when: the value is identical for every caller · the cache is partitioned per user or tenant.
Report as: `<cache>` keys `<value>` on `<key>`, but it depends on `<identity input>`: user B is served user A's `<data>`; fix: add `<input>` to the key, partition the cache per tenant or user, or stop caching it.
