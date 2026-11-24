# implicit-coercion
langs: typescript

When: `==`/`!=` with non-null operands, `+` on values that may be strings, or `Number()`/`parseInt`/`parseFloat`/`isNaN` applied to input.
Ask: What arrives at runtime? Query params, form fields, env vars, headers and CSV cells are strings even when typed `number`. `"5" + 1` is `"51"`; `Number("")` and `Number(null)` are `0`; `parseInt("10px")` is `10`; `isNaN("abc")` coerces (`Number.isNaN` does not); `0 == ""` and `"0" == false` are true.
Not when: `x == null` deliberately matching `null` and `undefined`.
Report as: `<expr>` at `<site>` coerces `<input>`: `<"" becomes 0 / strings concatenate>`, so `<wrong amount / invalid input accepted>`; fix: parse to a number once at the boundary, rejecting `NaN` and empty, then compare with `===`.
