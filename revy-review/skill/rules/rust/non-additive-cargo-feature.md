# non-additive-cargo-feature
langs: rust

When: Cargo feature, or `#[cfg(feature = …)]` / `#[cfg(not(feature = …))]` code, added or changed in a library crate.
Ask: Cargo enables the union of the features any crate in the graph requests: does every combination still compile and behave, this one with any other included? Does enabling it remove or change items — `cfg(not(feature))`, `compile_error!` for two features together, a gated `pub` field or required trait method? Is `no_std` a subtractive feature instead of an additive `std`?
Not when: a binary crate nobody depends on.
Report as: feature `<f>` is not additive: once any crate in the graph enables it, `<code that built without it>` breaks; fix: features that only add items, an additive `std` feature with `#![cfg_attr(not(feature = "std"), no_std)]`, runtime selection for exclusive backends.
