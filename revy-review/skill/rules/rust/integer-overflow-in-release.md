# integer-overflow-in-release
langs: rust

When: Arithmetic or `as` casts added on integers from input, lengths, counters, money or time.
Ask: Can an operand leave the type's range (unsigned `len - 1` on empty)? Release builds wrap silently unless `overflow-checks` is set in `Cargo.toml`; `as` between integers truncates in every build, and `as` from a float saturates and turns NaN into 0.
Report as: `<expr>` overflows when `<condition>`: release builds silently produce `<value>`; fix: `checked_*` with an error, `try_from`, or a wider type.
