# test-depends-on-unpinned-input
langs: any

When: Test uses random or generated values, the current time, fresh UUIDs, unordered-collection order, or state shared with other tests.
Ask: Does an assertion or a branch under test depend on it — a random enum picking another branch, a date crossing month end or DST, set or hash order inside an expected list, rows left by another test? Is the seed fixed and printed, the clock frozen, the result sorted before comparison, the state created per test?
Report as: `<test>` depends on `<unpinned input>`: it fails only when `<condition>`, and the failure cannot be replayed; fix: hardcode `<value>`, freeze the clock, sort, or isolate the state.
