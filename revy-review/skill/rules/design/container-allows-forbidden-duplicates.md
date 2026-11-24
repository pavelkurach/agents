# container-allows-forbidden-duplicates
langs: any

When: Keyed or ordered data held as a list of pairs, a list used as a set, or parallel lists, where the domain forbids duplicates or misalignment.
Ask: What does each consumer do with a duplicate key or lists of different length? `dict(pairs)` keeps the last, a `next(…)` search keeps the first, `zip` truncates silently (`zip(strict=True)` raises). Do two consumers resolve it differently?
Not when: duplicates are valid in the domain (repeated headers or query parameters).
Report as: `<container>` can hold `<duplicate / misaligned>` entries, and `<consumer A>` and `<consumer B>` resolve them differently: `<consequence>`; fix: a map, set or list of records whose shape rules it out.
