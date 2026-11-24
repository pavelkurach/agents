# validation-interleaved-with-writes
langs: any

When: Input validated piece by piece while acting on it, so a later check can fail after writes, sends or mutations.
Ask: Take the last check: if it fails, what has already been written, sent or mutated, and is it undone? Can every check run before the first effect?
Not when: all effects sit in one transaction that the failure rolls back.
Report as: `<late check>` can fail after `<earlier effect>`: invalid input leaves `<partial state>`; fix: parse the whole input into a checked value first, then act.
