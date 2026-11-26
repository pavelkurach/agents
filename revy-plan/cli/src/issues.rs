//! Every independent defect a check finds, each located by a JSON pointer.
//!
//! Checks that can find more than one defect collect them into an [`Issues`]
//! instead of stopping at the first, so a single `apply`/`check` reports the whole
//! list. A check builds a [`Collector`], pushes each defect at its pointer, and
//! finishes into `Result<_, Issues<E>>`; a caller that nests one document inside
//! another re-roots the pointers with [`Issues::under`].

use std::fmt;

use crate::primitives::OneOrMore;

/// An RFC 6901 JSON pointer into the document a check examined. The empty
/// pointer names the document root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct JsonPointer(String);

impl JsonPointer {
    /// The document root.
    pub const fn root() -> Self {
        Self(String::new())
    }

    /// A pointer from already-escaped `/`-prefixed text built by this crate.
    pub(crate) fn new(pointer: impl Into<String>) -> Self {
        Self(pointer.into())
    }

    /// This pointer extended by one segment, escaped per RFC 6901.
    #[must_use]
    pub fn join(&self, segment: impl fmt::Display) -> Self {
        let escaped = segment.to_string().replace('~', "~0").replace('/', "~1");
        Self(format!("{}/{escaped}", self.0))
    }

    /// This pointer re-rooted beneath `prefix`.
    #[must_use]
    pub fn under(&self, prefix: &Self) -> Self {
        Self(format!("{}{}", prefix.0, self.0))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JsonPointer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            formatter.write_str("/")
        } else {
            formatter.write_str(&self.0)
        }
    }
}

/// One defect and where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located<E> {
    pointer: JsonPointer,
    error: E,
}

impl<E> Located<E> {
    pub const fn new(pointer: JsonPointer, error: E) -> Self {
        Self { pointer, error }
    }

    pub const fn pointer(&self) -> &JsonPointer {
        &self.pointer
    }

    pub const fn error(&self) -> &E {
        &self.error
    }

    pub fn into_error(self) -> E {
        self.error
    }
}

/// A root-pointer defect concerns the whole document, so it prints bare.
impl<E: fmt::Display> fmt::Display for Located<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.pointer.as_str().is_empty() {
            write!(formatter, "{}", self.error)
        } else {
            write!(formatter, "{}: {}", self.pointer, self.error)
        }
    }
}

/// A non-empty list of located defects, in the order they were found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issues<E>(OneOrMore<Located<E>>);

impl<E> Issues<E> {
    /// A single defect.
    pub fn one(pointer: JsonPointer, error: E) -> Self {
        Self(OneOrMore::from_one(Located::new(pointer, error)))
    }

    /// The first defect found.
    pub fn first(&self) -> &Located<E> {
        self.0.first()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Located<E>> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.as_slice().len()
    }

    /// Always false: an `Issues` holds at least one defect.
    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Every pointer re-rooted beneath `prefix`.
    #[must_use]
    pub fn under(self, prefix: &JsonPointer) -> Self {
        Self(self.0.map_into(|located| Located {
            pointer: located.pointer.under(prefix),
            error: located.error,
        }))
    }

    /// Every defect converted into another error type, pointers kept.
    pub fn map<F>(self, convert: impl Fn(E) -> F) -> Issues<F> {
        Issues(self.0.map_into(|located| Located {
            pointer: located.pointer,
            error: convert(located.error),
        }))
    }

    pub fn into_vec(self) -> Vec<Located<E>> {
        self.0.into_vec()
    }
}

impl<E: fmt::Display> fmt::Display for Issues<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.len();
        if count > 1 {
            writeln!(formatter, "{count} problems:")?;
        }
        for (position, located) in self.iter().enumerate() {
            if position > 0 {
                writeln!(formatter)?;
            }
            if count > 1 {
                write!(formatter, "  - {located}")?;
            } else {
                write!(formatter, "{located}")?;
            }
        }
        Ok(())
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for Issues<E> {}

impl<'a, E> IntoIterator for &'a Issues<E> {
    type Item = &'a Located<E>;
    type IntoIter = std::slice::Iter<'a, Located<E>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Accumulates defects while a check walks a document.
#[derive(Debug)]
pub struct Collector<E>(Vec<Located<E>>);

impl<E> Default for Collector<E> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<E> Collector<E> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, pointer: JsonPointer, error: E) {
        self.0.push(Located::new(pointer, error));
    }

    /// Keep every defect of a sub-check, re-rooted beneath `prefix`.
    pub fn extend(&mut self, issues: Issues<E>, prefix: &JsonPointer) {
        self.0.extend(issues.under(prefix).into_vec());
    }

    /// Run a sub-check, keeping its value on success and its defects otherwise.
    pub fn absorb<T>(&mut self, result: Result<T, Issues<E>>, prefix: &JsonPointer) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(issues) => {
                self.extend(issues, prefix);
                None
            }
        }
    }

    pub const fn is_clean(&self) -> bool {
        self.0.is_empty()
    }

    /// `value` when nothing was collected, every collected defect otherwise.
    pub fn finish<T>(self, value: T) -> Result<T, Issues<E>> {
        OneOrMore::parse(self.0).map_or_else(|_| Ok(value), |issues| Err(Issues(issues)))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn a_clean_collector_finishes_with_its_value() {
        let collector: Collector<&str> = Collector::new();

        let outcome = collector.finish(7);

        assert_eq!(outcome, Ok(7));
    }

    #[test]
    fn a_collector_keeps_every_defect_in_order() {
        let mut collector = Collector::new();
        collector.push(JsonPointer::new("/a"), "first");
        collector.push(JsonPointer::new("/b"), "second");

        let issues = collector.finish(()).expect_err("two defects");

        assert_eq!(
            issues.to_string(),
            "2 problems:\n  - /a: first\n  - /b: second"
        );
    }

    #[test]
    fn extended_defects_are_re_rooted_beneath_the_prefix() {
        let mut collector = Collector::new();
        collector.extend(
            Issues::one(JsonPointer::new("/coverage/0"), "dup"),
            &JsonPointer::new("/exploration"),
        );

        let issues = collector.finish(()).expect_err("one defect");

        assert_eq!(issues.first().pointer().as_str(), "/exploration/coverage/0");
    }

    #[test]
    fn joining_a_segment_escapes_slash_and_tilde() {
        let pointer = JsonPointer::root().join("a/b~c");

        assert_eq!(pointer.as_str(), "/a~1b~0c");
    }

    #[test]
    fn a_single_defect_displays_without_a_count() {
        let issues = Issues::one(JsonPointer::new("/a"), "broken");

        assert_eq!(issues.to_string(), "/a: broken");
    }

    #[test]
    fn a_defect_at_the_root_displays_without_a_pointer() {
        let issues = Issues::one(JsonPointer::root(), "broken");

        assert_eq!(issues.to_string(), "broken");
    }
}
