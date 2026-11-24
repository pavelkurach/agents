# object-as-dictionary-with-external-keys
langs: typescript

When: Plain object indexed by a key from input (`obj[key]`, `key in obj`), or parsed JSON deep-merged or assigned into objects.
Ask: Can the key be `__proto__`, `constructor` or `toString`? Lookups find inherited members (`"toString" in {}` is true), assigning `obj["__proto__"]` replaces that object's prototype, and a recursive merge through a `__proto__` key writes onto `Object.prototype` for the whole process. Is it a `Map` or `Object.create(null)`, or checked with `Object.hasOwn`?
Report as: `<object>` keyed by `<input>` at `<site>` `<returns inherited members / allows prototype pollution>`: `<wrong lookup / attacker-set properties on every object>`; fix: a `Map` or null-prototype object, `Object.hasOwn`, and skipping `__proto__`, `constructor` and `prototype` keys in merges.
