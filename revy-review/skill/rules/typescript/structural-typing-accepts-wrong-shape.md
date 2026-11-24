# structural-typing-accepts-wrong-shape
langs: typescript

When: Signature taking several same-typed ids or units, an options object passed through a variable, or a callback typed with method syntax.
Ask: Would a wrong argument compile? Distinct ids that are all `string`, or amounts in different units, are interchangeable — check the order at each call site. Excess-property checks apply only to fresh object literals, so a misspelled option held in a variable is ignored. Method-shorthand members (`m(x: T): void`) are checked bivariantly even under `strictFunctionTypes`.
Report as: `<signature>` at `<site>` structurally accepts `<wrong value>`: `<swapped ids / an ignored option / a narrower callback>` compiles and `<consequence>`; fix: branded types for ids and units, `satisfies` on the options literal, function-property syntax for callbacks.
