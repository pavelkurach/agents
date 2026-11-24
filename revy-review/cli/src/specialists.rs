//! Specialists: `reviewers/<name>.md` in the skill and in `~/.revy-review/`.
//!
//! A file may start with `rules: <folder>, …`, the rule folders whose rules the specialist
//! receives (without it, none); the rest is its prompt, read on top of `reviewer.md`. The
//! file stem is its name.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::model::{Diagnostic, repeats};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Specialist {
    pub name: String,
    pub path: PathBuf,
    /// The rule folders (topics) whose rules it receives; empty: no rules.
    pub rule_folders: BTreeSet<String>,
}

pub fn parse(name: &str, path: PathBuf, text: &str) -> Result<Specialist, String> {
    let mut lines = text
        .lines()
        .skip_while(|line| line.trim().is_empty())
        .peekable();
    let rule_folders: BTreeSet<String> = match lines.next_if(|line| line.starts_with("rules:")) {
        None => BTreeSet::new(),
        Some(line) => {
            let folders: BTreeSet<String> = line
                .trim_start_matches("rules:")
                .split(',')
                .map(str::trim)
                .filter(|folder| !folder.is_empty())
                .map(str::to_owned)
                .collect();
            if folders.is_empty() {
                return Err(format!(
                    "specialist {name}: `rules:` names no rule folder; list the folders it receives, or drop the line for none"
                ));
            }
            folders
        }
    };
    if lines.all(|line| line.trim().is_empty()) {
        return Err(format!(
            "specialist {name} has no prompt; write what it reviews"
        ));
    }
    Ok(Specialist {
        name: name.to_owned(),
        path,
        rule_folders,
    })
}

/// Specialists sharing a name, within or across sources, which the owner could not tell apart.
pub fn duplicates(specialists: &[Specialist]) -> Vec<Diagnostic> {
    repeats(specialists, |specialist| specialist.name.clone())
        .into_iter()
        .filter_map(|(index, first)| Some((specialists.get(index)?, specialists.get(first)?)))
        .map(|(specialist, first)| {
            Diagnostic::new(
                specialist.path.display().to_string(),
                format!(
                    "specialist {} is also {}",
                    specialist.name,
                    first.path.display()
                ),
            )
        })
        .collect()
}

/// A `rules:` folder no rules directory has, which would silently narrow the index to nothing.
pub fn unknown_rule_folders(
    specialists: &[Specialist],
    topics: &BTreeSet<&str>,
) -> Vec<Diagnostic> {
    specialists
        .iter()
        .flat_map(|specialist| {
            specialist
                .rule_folders
                .iter()
                .filter(|folder| !topics.contains(folder.as_str()))
                .map(|folder| {
                    Diagnostic::new(
                        specialist.path.display().to_string(),
                        format!(
                            "specialist {} names rule folder `{folder}`, which no rules directory has",
                            specialist.name
                        ),
                    )
                })
        })
        .collect()
}

/// `Reviewer <n>: <name> — <file>` per specialist, numbered on from `first`, with the rules
/// index of its `rules:` folders, from `index`, indented below (none without them).
pub fn index(
    specialists: &[Specialist],
    first: u32,
    index: impl Fn(&BTreeSet<String>) -> String,
) -> String {
    let mut out = String::new();
    for (n, specialist) in (first..).zip(specialists) {
        let _ = writeln!(
            out,
            "Reviewer {n}: {} — {}",
            specialist.name,
            specialist.path.display()
        );
        if !specialist.rule_folders.is_empty() {
            for line in index(&specialist.rule_folders).lines() {
                let _ = writeln!(out, "  {line}");
            }
        }
    }
    out
}
