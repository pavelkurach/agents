# mock-keeps-old-contract
langs: any

When: What a function returns, raises or accepts, or the side effect it performs, changed.
Ask: Search tests for the function in patch targets, mock setups, fakes or test implementations of its interface or trait, and recorded responses. Does each still return, raise or accept the old shape? Is the mock derived from the real signature (`autospec`, `spec_set`, a real trait impl), so the change fails it? A test asserting the old behaviour directly should now fail: if CI is green, why (skipped, not collected, mocked)?
Report as: `<test>` fakes `<fn>` with the old `<contract>`, so `<caller>` is tested against behaviour that no longer exists and stays green; fix: update the fake to the new contract and derive it from the real object, or call the real function.
