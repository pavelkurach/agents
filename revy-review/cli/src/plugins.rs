//! Plug-in reviewers `~/.revy-review/reviewers/<name>.md`: the file is the reviewer's
//! prompt, read on top of `reviewer.md`; the file stem is its name.

use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reviewer {
    pub name: String,
    pub path: PathBuf,
}

/// The reviewer a plug-in file defines; a blank file has nothing to review for.
pub fn parse(name: &str, path: PathBuf, text: &str) -> Result<Reviewer, String> {
    if text.trim().is_empty() {
        return Err(format!(
            "plug-in reviewer {name} is blank; write its prompt: what it reviews and goes deepest on"
        ));
    }
    Ok(Reviewer {
        name: name.to_owned(),
        path,
    })
}

/// Each plug-in reviewer with its reviewer number, counting on from `first`.
pub fn numbered(reviewers: &[Reviewer], first: u32) -> impl Iterator<Item = (u32, &Reviewer)> {
    (first..).zip(reviewers)
}

/// `Reviewer <n>: <name> — <file>` per plug-in reviewer, numbered on from `first`.
pub fn index(reviewers: &[Reviewer], first: u32) -> String {
    let mut out = String::new();
    for (n, reviewer) in numbered(reviewers, first) {
        let _ = writeln!(
            out,
            "Reviewer {n}: {} — {}",
            reviewer.name,
            reviewer.path.display()
        );
    }
    out
}
