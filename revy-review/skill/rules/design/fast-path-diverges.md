# fast-path-diverges
langs: any

When: Second implementation of one computation added beside the general one: fast path, cache, SIMD or platform-specific branch, specialized query.
Ask: Do both return the same result on every input the fast path accepts? Compare at the edges: empty, odd or unaligned lengths, unicode, negative, NaN, the size threshold itself. Does the gate (size check, `cfg`, feature flag, CPU detection) admit exactly the inputs the fast path handles? Does a test run both paths on the same inputs, and does CI reach the path its platform skips by default? Is the gain measured on realistic input?
Report as: `<fast path>` and `<general path>` disagree on `<input>`: the result depends on `<size / platform / cache state>`, and the divergent path ships untested; fix: a differential test of both paths over shared edge inputs, a gate matching the fast path's domain, or drop a fast path with no measured gain.
