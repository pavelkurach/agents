# expects-any-failure
langs: any

When: Test expects an error or panic but accepts any: a base exception type, a bare panic expectation, or only an is-error check.
Ask: Look for `pytest.raises(Exception)` without `match`, `#[should_panic]` without `expected`, `assert!(r.is_err())`, `toThrow()` with no argument. Could another failure satisfy it — a typo, a missing fixture, a `None` access, an unrelated `unwrap` before the code under test? Does the test check the type or variant and the detail that identifies the cause?
Report as: `<test>` accepts any `<error / panic>`, so it passes when `<fn>` fails for an unrelated reason; fix: assert the specific type or variant (`pytest.raises(X, match=…)`, `assert!(matches!(r, Err(E::Y)))`).
