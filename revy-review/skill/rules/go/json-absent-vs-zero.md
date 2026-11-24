# json-absent-vs-zero
langs: go

When: JSON struct tags (`omitempty`) added or changed, JSON decoded into a struct, `map[string]any` or `any`, or a value reused across decodes.
Ask: Can absent be told from `false`, `0` or `""`? Only a pointer field can. `omitempty` drops a legitimate `false` or `0` and never omits a zero struct such as `time.Time` (Go 1.24+ `omitzero` does). Decoding ignores unknown fields, matches keys case-insensitively, keeps a reused struct's fields the input lacks, and turns numbers in `any` into `float64` (ids above 2^53 corrupt).
Report as: `<field>` at `<site>` conflates `<absent>` with `<zero>`: `<a PATCH resets it / a false flag disappears / a large id is rounded>`; fix: a pointer or `omitzero`, a fresh value per decode, `Decoder.UseNumber` or typed fields, `DisallowUnknownFields` for strict inputs.
