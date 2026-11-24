# patch-target-not-lookup-site
langs: python

When: Test patches a dotted path with `mock.patch`, `mocker.patch` or `monkeypatch.setattr`.
Ask: Does the code under test look the name up at that path? After `from a import f`, module `b` holds its own binding, so patching `a.f` leaves `b.f` real. With the wrong target, does the test still pass because the real call succeeds or the mock is never asserted?
Report as: `<test>` patches `<path>`, but `<module>` calls its own imported `<name>`, so the real one runs and the test passes by coincidence; fix: patch `<module>.<name>`, or inject the dependency.
Example:
  bad:  # orders.py does `from billing import charge`
        patch("billing.charge")
  good: patch("orders.charge")
