//! What `revy-review check` knows about the repository: the change (changed files and
//! the head-side lines a forge shows) and a snapshot of files at head.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::model::{RepoPath, Site};

/// How far from its site a quote may sit.
const QUOTE_SLACK: u32 = 3;

/// Changed files that exist at head, each with the head-side line ranges of
/// its hunks (context included, as a forge diff shows them).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Change {
    files: BTreeMap<RepoPath, Vec<(u32, u32)>>,
}

impl Change {
    /// From `git diff --name-only --diff-filter=d` output and the unified diff of the same range.
    pub fn parse(names: &str, unified: &str) -> Self {
        let mut files: BTreeMap<RepoPath, Vec<(u32, u32)>> = names
            .lines()
            .filter_map(|name| name.parse().ok())
            .map(|path| (path, Vec::new()))
            .collect();
        let mut current: Option<RepoPath> = None;
        for line in unified.lines() {
            if let Some(target) = line.strip_prefix("+++ ") {
                current = target.strip_prefix("b/").and_then(|path| path.parse().ok());
            } else if let (Some(path), Some(range)) = (&current, hunk_range(line))
                && let Some(ranges) = files.get_mut(path)
            {
                ranges.push(range);
            }
        }
        Self { files }
    }

    pub fn files(&self) -> impl Iterator<Item = &RepoPath> {
        self.files.keys()
    }

    /// Whether a forge would place an inline comment on this site.
    pub fn shows(&self, site: &Site) -> bool {
        let line = site.line.get();
        self.files.get(&site.path).is_some_and(|ranges| {
            ranges
                .iter()
                .any(|&(start, end)| (start..=end).contains(&line))
        })
    }
}

/// `@@ -a,b +c,d @@` → head lines `c..=c+d-1`; `None` for a pure deletion.
fn hunk_range(line: &str) -> Option<(u32, u32)> {
    let new_side = line
        .strip_prefix("@@ -")?
        .split(' ')
        .nth(1)?
        .strip_prefix('+')?;
    let (start, count) = match new_side.split_once(',') {
        Some((start, count)) => (start.parse::<u32>().ok()?, count.parse::<u32>().ok()?),
        None => (new_side.parse::<u32>().ok()?, 1),
    };
    let last = start.checked_add(count.checked_sub(1)?)?;
    Some((start, last))
}

/// File contents at head for the paths a document cites; `None` = absent at head.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeadFiles {
    files: BTreeMap<RepoPath, Option<Vec<String>>>,
}

impl HeadFiles {
    pub fn new(files: BTreeMap<RepoPath, Option<String>>) -> Self {
        let files = files
            .into_iter()
            .map(|(path, text)| {
                (
                    path,
                    text.map(|text| text.lines().map(str::to_owned).collect()),
                )
            })
            .collect();
        Self { files }
    }

    fn lines(&self, path: &RepoPath) -> Result<&[String], String> {
        match self.files.get(path) {
            Some(Some(lines)) => Ok(lines),
            _ => Err(format!("{path} does not exist at head")),
        }
    }

    /// The site names an existing line at head.
    pub fn resolve(&self, site: &Site) -> Result<(), String> {
        let lines = self.lines(&site.path)?;
        if usize::try_from(site.line.get()).is_ok_and(|line| line <= lines.len()) {
            Ok(())
        } else {
            Err(format!(
                "line {} is past the end of {} ({} lines)",
                site.line,
                site.path,
                lines.len()
            ))
        }
    }

    /// `quote` is a single non-blank line found within ±3 lines of `site`.
    pub fn quote_near(&self, site: &Site, quote: &str) -> Result<(), String> {
        let wanted = quote.trim();
        if wanted.is_empty() {
            return Err("quote is blank; copy the exact line at the first site".to_owned());
        }
        if wanted.contains('\n') {
            return Err(
                "quote spans several lines; copy the one line at the first site".to_owned(),
            );
        }
        let lines = self.lines(&site.path)?;
        let first = site.line.get().saturating_sub(QUOTE_SLACK).max(1);
        let last = site.line.saturating_add(QUOTE_SLACK).get();
        let found = (first..=last)
            .filter_map(NonZeroU32::new)
            .filter_map(|line| lines.get(usize::try_from(line.get().checked_sub(1)?).ok()?))
            .any(|line| line.contains(wanted));
        if found {
            Ok(())
        } else {
            Err(format!(
                "quote `{wanted}` is not within ±3 lines of {site} at head"
            ))
        }
    }
}
