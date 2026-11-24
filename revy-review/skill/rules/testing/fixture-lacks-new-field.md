# fixture-lacks-new-field
langs: any

When: Field added to a model, schema or struct that production always sets, or new logic branching on a field's value.
Ask: Find every way tests build the type: factories, JSON or YAML fixtures, dicts spread into the constructor, `..Default::default()` or builder defaults, recorded payloads. Which leave the field unset, so a default or `.get()` fallback runs a shape production never produces? Does some test set each value the new logic branches on?
Not when: every construction goes through a typed constructor that fails to compile or type-check without the field.
Report as: `<fixture>` omits `<field>`, so tests run `<fn>` with `<default>`, which production never sends, and `<branch>` is never exercised; fix: set a realistic `<field>` in the shared factory and add a case per value the logic branches on.
