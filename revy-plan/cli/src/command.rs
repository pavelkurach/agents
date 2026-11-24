//! The command a runtime-evidence gathering step executes.
//!
//! This is the tool-invocation command captured as [`RuntimeEvidence.command`] —
//! a program with arguments, or a shell script.
//!
//! [`RuntimeEvidence.command`]: crate::evidence::RuntimeEvidence

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::primitives::NonEmptyText;

/// A command a probe or runtime-evidence step runs: a program with arguments, or
/// a shell script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandSpec {
    Program {
        program: NonEmptyText,
        args: Vec<String>,
    },
    Shell {
        script: NonEmptyText,
    },
}
