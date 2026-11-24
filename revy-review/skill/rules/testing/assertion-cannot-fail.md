# assertion-cannot-fail
langs: any

When: Test assertion added or changed.
Ask: Can it fail when production is wrong? Check that it runs: outside a loop over a possibly empty result and outside an `except` branch, in a coroutine that is awaited, in a function the runner collects. Check that the expected value is a literal, rather than computed by the code under test or read back from the mock that supplied it. Check that inputs meant to differ are distinct (both ids `1`, every amount `0`, a default equal to the expected value make them coincide).
Report as: `<test>` passes whatever `<fn>` does because `<reason>`; fix: `<assert a fixed literal / use distinct inputs / assert the collection is non-empty first>`.
