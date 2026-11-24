# int64-through-js-number
langs: typescript

When: 64-bit values (database or snowflake ids, counters, µs or ns timestamps) read from JSON into `number`, or a `bigint` serialized.
Ask: Can values exceed `Number.MAX_SAFE_INTEGER` (2^53 − 1)? `JSON.parse` rounds them silently; `JSON.stringify` throws `TypeError` on a `bigint`. What does the producer emit — read its schema for int64 fields.
Report as: `<field>` at `<site>` passes through a JS `number`: values above 2^53 are rounded, so `<ids collide / lookups miss>`; fix: transport them as strings and keep them as `string` or `bigint`.
