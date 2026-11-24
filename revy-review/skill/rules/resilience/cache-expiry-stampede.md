# cache-expiry-stampede
langs: any

When: Read-through cache (get, compute on miss, set) or its TTL added or changed for a key that many requests read.
Ask: When a hot key expires, how many concurrent misses recompute it — is there single-flight, a lock, or stale-while-revalidate? Do many keys get the same TTL at once (bulk warm-up, deploy) with no jitter? Can the origin absorb that synchronized burst?
Not when: the recompute is cheap · the key is rarely read.
Report as: `<cache>` sends every concurrent miss on `<key>` to `<origin>`: at expiry the origin takes the full request rate at once and can fall over; fix: single-flight per key, stale-while-revalidate, jittered TTLs.
