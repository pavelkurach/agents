# backtracking-regex-on-input
langs: any

When: Regular expression added or changed that runs on client-supplied text with a backtracking engine (Python `re`, JS `RegExp`, Java, .NET, PCRE).
Ask: Does the pattern nest or overlap quantifiers — `(a+)+`, `(\w+\s?)*`, `(a|aa)*`, two adjacent `.*` or `\s*` that can match the same text before a tail that fails — so a crafted non-matching input takes exponential or polynomial time? Is the input length capped before matching? Is a linear-time engine at hand: Rust `regex`, Go `regexp` and the `google-re2` binding guarantee it; Rust `fancy-regex` and `pcre2` do not.
Not when: a linear-time engine runs the pattern · the input is capped at a length where the worst case is cheap.
Report as: `<pattern>` at `<site>` backtracks catastrophically on `<crafted input>`: one request pins a CPU `<for seconds / indefinitely>` and stalls the worker (ReDoS); fix: rewrite the pattern without ambiguous repetition, cap the input length, or use a linear-time engine.
