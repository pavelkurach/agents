//! `revy-review check`: every file in the review directory, and the accounting across them.

use std::collections::BTreeSet;

use crate::diff::{Change, HeadFiles};
use crate::investigations::{self, Investigations};
use crate::model::{Diagnostic, RepoPath, State};
use crate::problem::{self, Problem};
use crate::review::{self, Review};
use crate::verified::{self, Verified};

/// Review numbers from here on belong to checks and the investigator, not to reviewer lenses.
const FIRST_CHECK: u32 = 5;

/// The parsed files of one review directory; absent files are `None` or empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewDir {
    pub state: State,
    pub problem: Option<Problem>,
    /// `(reviews/<n>.json, review)` in file-name order.
    pub reviews: Vec<(String, Review)>,
    pub investigations: Option<Investigations>,
    pub verified: Option<Verified>,
}

impl ReviewDir {
    pub fn cited_paths(&self) -> BTreeSet<RepoPath> {
        let problem = self.problem.iter().flat_map(Problem::cited_paths);
        let reviews = self
            .reviews
            .iter()
            .flat_map(|(_, review)| review.cited_paths());
        let verified = self.verified.iter().flat_map(Verified::cited_paths);
        problem.chain(reviews).chain(verified).cloned().collect()
    }

    /// One `<file>: ok (…)` line per file checked.
    pub fn summary(&self) -> String {
        let problem = self.problem.iter().map(|problem| {
            format!(
                "problem.json: ok ({} criteria, blocks: {})\n",
                problem.criteria.len(),
                problem.blocks
            )
        });
        let reviews = self.reviews.iter().map(|(file, review)| {
            format!(
                "{file}: ok ({} findings, {} ledger entries)\n",
                review.findings.len(),
                review.ledger.len()
            )
        });
        let investigations = self
            .investigations
            .iter()
            .map(|doc| format!("{}: ok ({} items)\n", investigations::FILE, doc.items.len()));
        let verified = self.verified.iter().map(|verified| {
            format!(
                "verified.json: ok ({} kept, {} dropped)\n",
                verified.findings.len(),
                verified.dropped.len()
            )
        });
        problem
            .chain(reviews)
            .chain(investigations)
            .chain(verified)
            .collect()
    }
}

pub fn check(
    dir: &ReviewDir,
    rules: &BTreeSet<String>,
    change: &Change,
    head: &HeadFiles,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    if let Some(problem) = &dir.problem {
        errors.extend(problem::check("problem.json", problem, head));
    }
    let reviewers = dir.state.reviewers.count();
    for (file, review) in &dir.reviews {
        let stem = file
            .strip_prefix("reviews/")
            .and_then(|name| name.strip_suffix(".json"))
            .unwrap_or(file);
        if stem
            .parse::<u32>()
            .is_ok_and(|n| (reviewers + 1..FIRST_CHECK).contains(&n))
        {
            errors.push(Diagnostic::new(
                file,
                format!("state.json reviewers = {reviewers}, so only reviews/1.json..{reviewers}.json are reviewers and checks start at {FIRST_CHECK}.json"),
            ));
        }
        errors.extend(review::check(file, stem, review, rules, change, head));
    }
    if let Some(doc) = &dir.investigations {
        errors.extend(investigations::check(doc, &dir.reviews));
    }
    if let Some(verified) = &dir.verified {
        errors.extend(complete(dir));
        let expected = verified::expected(&dir.reviews, dir.problem.as_ref());
        errors.extend(verified::check(verified, &expected, rules, head));
    }
    errors
}

/// What the verifier needs before it runs: every reviewer lens's file (unless a blocking
/// gate stopped the review before any ran) and, when there are suspicions, their investigation.
fn complete(dir: &ReviewDir) -> Vec<Diagnostic> {
    let reviewers = dir.state.reviewers.count();
    let present: BTreeSet<String> = dir.reviews.iter().map(|(file, _)| file.clone()).collect();
    let lenses: Vec<String> = (1..=reviewers)
        .map(|n| format!("reviews/{n}.json"))
        .collect();
    let stopped_at_gate = dir.problem.as_ref().is_some_and(|problem| problem.blocks)
        && lenses.iter().all(|file| !present.contains(file));
    let missing_reviews = lenses
        .into_iter()
        .filter(|file| !stopped_at_gate && !present.contains(file))
        .map(|file| {
            Diagnostic::new(
                format!("{file} missing"),
                format!("state.json reviewers = {reviewers} needs reviews/1.json..{reviewers}.json, one reviewer subagent per lens"),
            )
        });
    let suspicions: Vec<String> = investigations::raised(&dir.reviews)
        .keys()
        .map(ToString::to_string)
        .collect();
    let missing_investigations =
        (dir.investigations.is_none() && !suspicions.is_empty()).then(|| {
            Diagnostic::new(
                investigations::FILE,
                format!(
                    "missing; reviews hold suspicions ({}) that the investigator must resolve",
                    suspicions.join(", ")
                ),
            )
        });
    missing_reviews.chain(missing_investigations).collect()
}
