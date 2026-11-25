//! Specialists: `reviewers/<name>.md` in the skill and in `~/.revy-review/`.
//!
//! A file may start with header lines, in any order: `rules: <folder>, …`, the rule folders
//! whose rules the specialist receives (without it, none), and `dispatch: always | when
//! relevant` (without it, always). The rest is its prompt, read on top of `reviewer.md`. The
//! file stem is its name.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::model::{Decision, Diagnostic, repeats};

/// Whether every review runs the specialist, or the coordinator decides per review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    Always,
    WhenRelevant,
}

impl Dispatch {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::WhenRelevant => "when relevant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Specialist {
    pub name: String,
    pub path: PathBuf,
    /// The rule folders (topics) whose rules it receives; empty: no rules.
    pub rule_folders: BTreeSet<String>,
    pub dispatch: Dispatch,
}

fn parse_rules(name: &str, value: &str) -> Result<BTreeSet<String>, String> {
    let folders: BTreeSet<String> = value
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
    Ok(folders)
}

fn parse_dispatch(name: &str, value: &str) -> Result<Dispatch, String> {
    [Dispatch::Always, Dispatch::WhenRelevant]
        .into_iter()
        .find(|dispatch| dispatch.as_str() == value.trim())
        .ok_or_else(|| {
            format!(
                "specialist {name}: `dispatch: {}` is not `always` or `when relevant`",
                value.trim()
            )
        })
}

pub fn parse(name: &str, path: PathBuf, text: &str) -> Result<Specialist, String> {
    let mut lines = text.lines().peekable();
    let mut rule_folders = None;
    let mut dispatch = None;
    loop {
        while lines.next_if(|line| line.trim().is_empty()).is_some() {}
        let Some(line) =
            lines.next_if(|line| line.starts_with("rules:") || line.starts_with("dispatch:"))
        else {
            break;
        };
        let (key, value) = line.split_once(':').unwrap_or((line, ""));
        let duplicate = match key {
            "rules" => rule_folders.replace(parse_rules(name, value)?).is_some(),
            _ => dispatch.replace(parse_dispatch(name, value)?).is_some(),
        };
        if duplicate {
            return Err(format!(
                "specialist {name}: `{key}:` appears twice; keep one"
            ));
        }
    }
    if lines.all(|line| line.trim().is_empty()) {
        return Err(format!(
            "specialist {name} has no prompt; write what it reviews"
        ));
    }
    Ok(Specialist {
        name: name.to_owned(),
        path,
        rule_folders: rule_folders.unwrap_or_default(),
        dispatch: dispatch.unwrap_or(Dispatch::Always),
    })
}

/// The specialists `state.json` records as dispatched, in load order, and its disagreements.
///
/// Errors: a decision for no loaded specialist, a specialist without one, a skipped
/// `dispatch: always` one. `reviewed`: some review file exists, so decisions must exist.
pub fn dispatched<'a>(
    specialists: &'a [Specialist],
    decisions: Option<&BTreeMap<String, Decision>>,
    reviewed: bool,
) -> (Vec<&'a Specialist>, Vec<Diagnostic>) {
    let Some(decisions) = decisions else {
        let errors = (reviewed && !specialists.is_empty())
            .then(|| {
                Diagnostic::new(
                    "state.json: specialists",
                    "missing; record a decision for every specialist `revy-review rules` lists before dispatching reviewers: `\"<name>\": \"dispatched\"` or `\"<name>\": {\"skipped\": \"<reason>\"}`",
                )
            })
            .into_iter()
            .collect();
        return (Vec::new(), errors);
    };
    let known: BTreeSet<&str> = specialists.iter().map(|s| s.name.as_str()).collect();
    let unknown = decisions
        .keys()
        .filter(|name| !known.contains(name.as_str()))
        .map(|name| {
            Diagnostic::new(
                format!("state.json: specialists.{name}"),
                "no such specialist; `revy-review rules` lists them",
            )
        });
    let undecided = specialists
        .iter()
        .filter(|specialist| !decisions.contains_key(&specialist.name))
        .map(|specialist| {
            Diagnostic::new(
                "state.json: specialists",
                format!(
                    "no decision for specialist {}; record it as dispatched or skipped",
                    specialist.name
                ),
            )
        });
    let skipped_always = specialists
        .iter()
        .filter(|specialist| specialist.dispatch == Dispatch::Always)
        .filter(|specialist| matches!(decisions.get(&specialist.name), Some(Decision::Skipped(_))))
        .map(|specialist| {
            Diagnostic::new(
                format!("state.json: specialists.{}", specialist.name),
                format!(
                    "skipped, but {} has `dispatch: always`; dispatch it",
                    specialist.path.display()
                ),
            )
        });
    let errors = unknown.chain(undecided).chain(skipped_always).collect();
    let chosen = specialists
        .iter()
        .filter(|specialist| matches!(decisions.get(&specialist.name), Some(Decision::Dispatched)))
        .collect();
    (chosen, errors)
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

/// `Specialist: <name> (dispatch: <mode>) — <file>` per specialist, with the rules index of
/// its `rules:` folders, from `index`, indented below (none without them).
pub fn index(specialists: &[Specialist], index: impl Fn(&BTreeSet<String>) -> String) -> String {
    let mut out = String::new();
    for specialist in specialists {
        let _ = writeln!(
            out,
            "Specialist: {} (dispatch: {}) — {}",
            specialist.name,
            specialist.dispatch.as_str(),
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
