//! Review comments overlaid on a rendered plan — the port of `render.py`'s
//! `--serve` comment affordance.
//!
//! A comment is a review overlay: it is stored on the final state *outside* the
//! candidate, so leaving one never changes the candidate fingerprint that keys
//! the plan's review packets (that fingerprint is taken over the candidate
//! alone). An open comment renders inline under its block; a resolved comment
//! moves to the plan's Discussion section.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::primitives::NonEmptyText;

/// A stable structural anchor to an authored block.
///
/// The section and block indices the renderer assigns in document order.
/// Structural rather than content-derived, so a comment stays put while its
/// block's text is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockAnchor {
    pub section: u32,
    pub block: u32,
}

impl BlockAnchor {
    #[must_use]
    pub const fn new(section: u32, block: u32) -> Self {
        Self { section, block }
    }

    /// The `data-block` token the renderer emits on a block and the serve POST
    /// echoes back — e.g. section 0, block 2 → `s0b2`.
    #[must_use]
    pub fn token(self) -> String {
        format!("s{}b{}", self.section, self.block)
    }

    /// Parse a `data-block` token back into an anchor at the serve boundary.
    /// Parse, don't validate: a malformed token yields `None` rather than a
    /// silently wrong anchor.
    #[must_use]
    pub fn parse_token(raw: &str) -> Option<Self> {
        let (section, block) = raw.strip_prefix('s')?.split_once('b')?;
        Some(Self {
            section: section.parse().ok()?,
            block: block.parse().ok()?,
        })
    }
}

/// One review comment on a block.
///
/// Open by default; resolving it moves it from its block to the Discussion
/// section. The text is the bare comment, exactly as `render.py` stored it —
/// no author, no timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Comment {
    pub anchor: BlockAnchor,
    pub text: NonEmptyText,
    #[serde(default)]
    pub resolved: bool,
}

impl Comment {
    /// A new, open comment on `anchor`.
    #[must_use]
    pub const fn open(anchor: BlockAnchor, text: NonEmptyText) -> Self {
        Self {
            anchor,
            text,
            resolved: false,
        }
    }
}

/// The open comments on one block, in the order they were left — the shape the
/// renderer wants for the inline blockquotes under a block.
#[must_use]
pub fn open_on(comments: &[Comment], anchor: BlockAnchor) -> Vec<&Comment> {
    comments
        .iter()
        .filter(|comment| !comment.resolved && comment.anchor == anchor)
        .collect()
}

/// Every resolved comment, in the order they were left — the Discussion
/// section's contents.
#[must_use]
pub fn resolved(comments: &[Comment]) -> Vec<&Comment> {
    comments.iter().filter(|comment| comment.resolved).collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_block_anchor_token_round_trips_through_parse() {
        let anchor = BlockAnchor::new(3, 12);
        assert_eq!(anchor.token(), "s3b12");
        assert_eq!(BlockAnchor::parse_token("s3b12"), Some(anchor));
    }

    #[test]
    fn test_parse_token_rejects_a_malformed_token() {
        assert_eq!(BlockAnchor::parse_token("3-12"), None);
        assert_eq!(BlockAnchor::parse_token("sXb1"), None);
        assert_eq!(BlockAnchor::parse_token("s1"), None);
    }

    #[test]
    fn test_comment_without_a_resolved_field_parses_as_open() {
        let json = r#"{"anchor":{"section":0,"block":1},"text":"double-locks here"}"#;
        let comment: Comment = serde_json::from_str(json).expect("comment parses");
        assert!(!comment.resolved);
    }

    #[test]
    fn test_open_on_selects_only_open_comments_for_the_block() {
        let text = NonEmptyText::parse("note").unwrap();
        let comments = vec![
            Comment::open(BlockAnchor::new(0, 0), text.clone()),
            Comment {
                anchor: BlockAnchor::new(0, 0),
                text: text.clone(),
                resolved: true,
            },
            Comment::open(BlockAnchor::new(0, 1), text),
        ];
        assert_eq!(open_on(&comments, BlockAnchor::new(0, 0)).len(), 1);
        assert_eq!(resolved(&comments).len(), 1);
    }
}
