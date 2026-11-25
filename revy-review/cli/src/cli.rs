//! Command shell: reads the review directory, the rules, the specialists and git,
//! calls the pure core, prints. Exit 0 = clean (warnings allowed), 1 = errors.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::check::{self, ReviewDir};
use crate::diff::Change;
use crate::git::{self, ShellError};
use crate::investigations;
use crate::model::{Diagnostic, State, parse_json};
use crate::report;
use crate::rules::{self, RuleSet};
use crate::specialists::{self, Specialist};

/// What a command produced: output, warnings, and errors that fail it.
#[derive(Debug, Default)]
struct Outcome {
    stdout: String,
    warnings: Vec<Diagnostic>,
    errors: Vec<Diagnostic>,
}

impl Outcome {
    fn invalid(errors: Vec<Diagnostic>) -> Self {
        Self {
            errors,
            ..Self::default()
        }
    }
}

fn finish(result: Result<Outcome, ShellError>) -> ExitCode {
    match result {
        Ok(outcome) => {
            print!("{}", outcome.stdout);
            for warning in &outcome.warnings {
                eprintln!("{}: warning: {}", warning.at, warning.message);
            }
            for error in &outcome.errors {
                eprintln!("{error}");
            }
            if outcome.errors.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

/// Validates every rule and specialist file and prints the index for the languages the
/// change touches.
pub fn rules(dir: &Path) -> ExitCode {
    finish(run_rules(dir))
}

/// Validates every file in the review directory and the accounting across them.
pub fn check(dir: &Path) -> ExitCode {
    finish(checked(dir).map(|result| match result {
        Ok((review_dir, _)) => Outcome {
            stdout: review_dir.summary(),
            ..Outcome::default()
        },
        Err(errors) => Outcome::invalid(errors),
    }))
}

/// Renders report.md from a clean review directory, or reads back the owner's edits.
pub fn sync(dir: &Path) -> ExitCode {
    finish(run_sync(dir))
}

/// Prints the post-ready comments from report.md.
pub fn comments(dir: &Path) -> ExitCode {
    finish(run_comments(dir))
}

/// Unwraps a parse or check result, or returns the command's failing outcome.
macro_rules! valid {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(errors) => return Ok(Outcome::invalid(errors)),
        }
    };
}

fn read_json<T: serde::de::DeserializeOwned>(
    dir: &Path,
    file: &str,
) -> Result<Option<Result<T, Diagnostic>>, ShellError> {
    let path = dir.join(file);
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(parse_json(file, &git::read(&path)?)))
}

fn read_state(dir: &Path) -> Result<Result<State, Vec<Diagnostic>>, ShellError> {
    let parsed = read_json(dir, "state.json")?.ok_or_else(|| {
        ShellError::Usage(format!(
            "{} has no state.json; run from the review directory or pass it",
            dir.display()
        ))
    })?;
    Ok(parsed.map_err(|error| vec![error]))
}

/// The parsed document, or `None` with its error recorded.
fn keep<T>(errors: &mut Vec<Diagnostic>, parsed: Option<Result<T, Diagnostic>>) -> Option<T> {
    parsed?.map_err(|error| errors.push(error)).ok()
}

/// Every file of the review directory, parsed; the parse errors of all that fail.
fn load(dir: &Path) -> Result<Result<ReviewDir, Vec<Diagnostic>>, ShellError> {
    let mut errors = Vec::new();
    let state = read_state(dir)?
        .map_err(|mut found| errors.append(&mut found))
        .ok();
    let problem = keep(&mut errors, read_json(dir, "problem.json")?);
    let investigations = keep(&mut errors, read_json(dir, investigations::FILE)?);
    let verified = keep(&mut errors, read_json(dir, "verified.json")?);
    let mut reviews = Vec::new();
    for path in git::files(&dir.join("reviews"), "json")? {
        let file = format!("reviews/{}", lossy(path.file_name()));
        if let Some(review) = keep(&mut errors, read_json(dir, &file)?) {
            reviews.push((file, review));
        }
    }
    Ok(match state {
        Some(state) if errors.is_empty() => Ok(ReviewDir {
            state,
            problem,
            reviews,
            investigations,
            verified,
        }),
        _ => Err(errors),
    })
}

fn lossy(name: Option<&OsStr>) -> String {
    name.map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Where rules and specialists load from, in order: the skill, then the owner's
/// plug-in folder `~/.revy-review/` when it exists; each may hold `rules/` and `reviewers/`.
fn sources(state: &State) -> Vec<PathBuf> {
    let plugin = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".revy-review"))
        .filter(|dir| dir.is_dir());
    std::iter::once(state.skill.as_path().to_path_buf())
        .chain(plugin)
        .collect()
}

/// Every rules directory, in load order; the skill's must exist.
fn rule_dirs(state: &State) -> Result<Vec<PathBuf>, ShellError> {
    let skill = state.skill.as_path();
    if !skill.join("rules").is_dir() {
        return Err(ShellError::Usage(format!(
            "skill directory {} (state.json `skill`) has no rules/ folder",
            skill.display()
        )));
    }
    Ok(sources(state)
        .into_iter()
        .map(|source| source.join("rules"))
        .filter(|dir| dir.is_dir())
        .collect())
}

/// `(topic, id, path)` of every `<topic>/<id>.md` in a rules directory.
fn rule_files(dir: &Path) -> Result<Vec<(String, String, PathBuf)>, ShellError> {
    let mut found = Vec::new();
    for topic in git::subdirectories(dir)? {
        for file in git::files(&topic, "md")? {
            found.push((lossy(topic.file_name()), lossy(file.file_stem()), file));
        }
    }
    Ok(found)
}

/// Every specialist, in load order (file-name order within a source), and the errors of
/// the files that define none.
fn specialists(state: &State) -> Result<(Vec<Specialist>, Vec<Diagnostic>), ShellError> {
    let mut found = Vec::new();
    let mut errors = Vec::new();
    for source in sources(state) {
        // README.md documents the folder; it is not a specialist.
        for path in git::files(&source.join("reviewers"), "md")?
            .into_iter()
            .filter(|p| p.file_name() != Some("README.md".as_ref()))
        {
            let text = git::read(&path)?;
            match specialists::parse(&lossy(path.file_stem()), path.clone(), &text) {
                Ok(specialist) => found.push(specialist),
                Err(message) => errors.push(Diagnostic::new(path.display().to_string(), message)),
            }
        }
    }
    Ok((found, errors))
}

/// The review directory, loaded and checked clean, with the change it reviews.
fn checked(dir: &Path) -> Result<Result<(ReviewDir, Change), Vec<Diagnostic>>, ShellError> {
    let review_dir = match load(dir)? {
        Ok(review_dir) => review_dir,
        Err(errors) => return Ok(Err(errors)),
    };
    let state = &review_dir.state;
    let repo = state.repo.as_path();
    let mut rules = BTreeSet::new();
    for rules_dir in rule_dirs(state)? {
        rules.extend(rule_files(&rules_dir)?.into_iter().map(|(_, id, _)| id));
    }
    // A broken specialist file is `revy-review rules`'s to report; nobody dispatches it.
    let (specialists, _) = specialists(state)?;
    let change = git::change(repo, &state.base, &state.head)?;
    let head = git::head_files(repo, &state.head, review_dir.cited_paths())?;
    let errors = check::check(&review_dir, &rules, &specialists, &change, &head);
    Ok(if errors.is_empty() {
        Ok((review_dir, change))
    } else {
        Err(errors)
    })
}

fn run_rules(dir: &Path) -> Result<Outcome, ShellError> {
    let state = valid!(read_state(dir)?);
    let mut sets = Vec::new();
    let mut errors = Vec::new();
    for rules_dir in rule_dirs(&state)? {
        let mut set = RuleSet {
            dir: rules_dir,
            rules: Vec::new(),
        };
        for (topic, id, path) in rule_files(&set.dir)? {
            match rules::parse(&topic, &id, &git::read(&path)?) {
                Ok(rule) => set.rules.push(rule),
                Err(message) => errors.push(Diagnostic::new(path.display().to_string(), message)),
            }
        }
        sets.push(set);
    }
    errors.extend(rules::duplicates(&sets));
    let (found, specialist_errors) = specialists(&state)?;
    errors.extend(specialist_errors);
    errors.extend(specialists::duplicates(&found));
    let topics: BTreeSet<&str> = sets
        .iter()
        .flat_map(|set| &set.rules)
        .map(|rule| rule.topic.as_str())
        .collect();
    errors.extend(specialists::unknown_rule_folders(&found, &topics));
    let specialist_index = specialists::index(&found, |folders| rules::index(&sets, Some(folders)));
    Ok(Outcome {
        stdout: rules::index(&sets, None) + &specialist_index,
        warnings: Vec::new(),
        errors,
    })
}

fn run_sync(dir: &Path) -> Result<Outcome, ShellError> {
    let (review_dir, change) = valid!(checked(dir)?);
    let Some(verified) = &review_dir.verified else {
        return Err(ShellError::Usage(
            "verified.json does not exist; sync renders the verifier's findings".to_owned(),
        ));
    };
    let path = dir.join(report::FILE);
    let rendered = !path.is_file();
    if rendered {
        let unresolved = review_dir
            .investigations
            .as_ref()
            .map(|doc| investigations::unresolved(&review_dir.reviews, doc))
            .unwrap_or_default();
        git::write(
            &path,
            &report::render(
                &review_dir.state.target,
                verified,
                &unresolved,
                &review_dir.state.skipped(),
            ),
        )?;
    }
    let report = valid!(report::parse(&git::read(&path)?));
    let summary = format!(
        "{} findings ({} to post), {} own findings",
        report.posted.len() + report.held,
        report.posted.len(),
        report.own.len()
    );
    Ok(Outcome {
        stdout: if rendered {
            format!("wrote {}: {summary}\n", report::FILE)
        } else {
            format!("{}: {summary}\n", report::FILE)
        },
        warnings: report::outside_diff(&report, &change),
        errors: Vec::new(),
    })
}

fn run_comments(dir: &Path) -> Result<Outcome, ShellError> {
    let path = dir.join(report::FILE);
    if !path.is_file() {
        return Err(ShellError::Usage(format!(
            "{} does not exist; run `revy-review sync` first",
            path.display()
        )));
    }
    let report = valid!(report::parse(&git::read(&path)?));
    Ok(Outcome {
        stdout: report::comments(&report),
        ..Outcome::default()
    })
}
