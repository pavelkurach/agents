# external-data-typed-without-parsing
langs: typescript

When: Data from outside the process (`JSON.parse`, `res.json()`, `req.body`, storage, `postMessage`) typed by annotation, `as`, or a generic `get<T>()`.
Ask: Does anything check the shape at runtime — a schema such as zod, or a type guard whose body tests every field it claims (a predicate's body is unchecked against its `x is T`)? `JSON.parse` and `res.json()` return `any`, which assigns to any type silently. Do typed fields survive JSON: `Date` arrives as `string`, `Map` and `Set` as `{}`, `undefined` keys vanish, `bigint` makes `JSON.stringify` throw?
Report as: `<source>` at `<site>` is asserted to be `<T>` without validation: `<malformed input / a string where a Date is typed>` flows on as typed and fails later at `<use>`; fix: parse at the boundary with a schema, and derive the type from it.
