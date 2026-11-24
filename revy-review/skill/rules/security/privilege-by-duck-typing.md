# privilege-by-duck-typing
langs: any

When: Privilege or bypass granted because an attribute, flag or method is present on an object (`getattr`, `hasattr`, a duck-typed field, optional chaining).
Ask: Which principal types can carry that attribute, including ones where it means something else or the user controls it? Does the check name the concrete principal type and role it means to allow?
Report as: `<check>` grants `<privilege>` to any object with `<attribute>`, so `<other principal>` gets it: `<consequence>`; fix: allowlist the concrete principal type and role explicitly.
