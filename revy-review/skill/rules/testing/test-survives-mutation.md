# test-survives-mutation
langs: any

When: Tests added or changed for code with comparisons, arithmetic, branches, filters, or several arguments of the same type.
Ask: Mutate the code under test, one change at a time: flip a comparison or move a boundary (`<` to `<=`), delete an early return or guard, skip a branch, swap two same-typed arguments (tenant and user id, source and destination), return the input unchanged. Which mutant passes every test? List the dimensions the code branches on — principal kind (anonymous, staff, app), variant, mode, empty or malformed input — and name the values no test reaches.
Not when: trivial getters or plain field mapping, where any mutation is a rewrite.
Report as: mutant `<mutation>` of `<fn>` passes all tests; missing case: `<input>` expecting `<outcome only the original produces>`; fix: add that case.
