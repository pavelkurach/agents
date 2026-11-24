# public-type-missing-common-traits
langs: rust

When: `pub` type, especially an error type, added to a library crate without the standard trait impls its callers need.
Ask: Error types: `Debug`, `Display`, `std::error::Error` with `source()`, and `Send + Sync + 'static`, so `?` into `anyhow` or `Box<dyn Error + Send + Sync>` works? Value types: `Debug`, `Clone`, `PartialEq`, `Default`, `From`/`TryFrom` where callers build, log or compare them? The orphan rule stops downstream crates adding these.
Not when: deliberately non-cloneable or non-comparable handles · secrets with a redacted `Debug`.
Report as: `<type>` lacks `<trait>`: callers cannot `<propagate it with ? / log / compare>` it, and the orphan rule stops them adding it; fix: derive or implement `<trait>`.
