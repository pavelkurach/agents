# public-api-semver-hazard
langs: rust

When: `pub` enum, struct, trait or function added or changed in a published library crate.
Ask: Is the change breaking — a removed or renamed item, a tightened bound, a new required trait method, a new variant or `pub` field on a type without `#[non_exhaustive]`, a generic method or `Self` return on a trait used as `dyn` (needs `where Self: Sized`)? Will a new type grow (then `#[non_exhaustive]` now), and is a trait not meant for outside impls sealed? Is the version bumped (`cargo semver-checks`)?
Not when: `publish = false`, or no external dependents.
Report as: `<item>` `<change>` breaks downstream `<match / struct literal / impl / dyn use>` without a major bump; fix: a default method, `#[non_exhaustive]`, a sealed trait, or a major version.
