# date-api-traps
langs: typescript

When: `Date` built from a string or parts, calendar arithmetic with `setX`, `toISOString` formatting, or a mutation of a `Date` that others hold.
Ask: Is the string ISO 8601 (other formats parse implementation-dependently)? Is an invalid date caught — `new Date("x")` does not throw: `getTime()` is `NaN`, comparisons are false, `toISOString()` throws `RangeError`? Months are 0-based and overflow rolls over (`setMonth(1)` on Jan 31 gives early March). `toISOString().slice(0, 10)` is the UTC date, which differs from the local one near midnight. `setX` mutates the object every holder shares.
Report as: `<expr>` at `<site>` `<misparses / overflows / shifts>` `<input>`: `<a date off by a day or month / NaN propagates>`; fix: a validated format or explicit `Date.UTC`, a `Number.isNaN(d.getTime())` check, and a copy before mutating.
