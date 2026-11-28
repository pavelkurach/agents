//! `revy-review check`: every file in the review directory, and the accounting across them.

use std::collections::BTreeSet;

use crate::diff::{Change, HeadFiles};
use crate::investigations::{self, Shards};
use crate::model::{Diagnostic, Level, RepoPath, Round, State};
use crate::problem::{self, Problem};
use crate::proof::{self, Cited, ProofObjects};
use crate::review::{self, Review};
use crate::specialists::{self, Specialist};
use crate::verified::{self, Verified};

/// How reviewers number: the depth's lens reviewers 1..=L review the whole change, the
/// dispatched specialists number on from L+1 in load order up to `last`, then the investigators.
#[derive(Debug, Clone, Copy)]
struct Roster {
    level: Level,
    last: u32,
}

impl Roster {
    fn new(level: Level, dispatched: &[&Specialist]) -> Self {
        let lens = level.lens_reviewers();
        let last = (lens + 1..).zip(dispatched).last().map_or(lens, |(n, _)| n);
        Self { level, last }
    }

    fn is_specialist(self, n: u32) -> bool {
        (self.level.lens_reviewers() + 1..=self.last).contains(&n)
    }

    /// Who reviewers 1..=last are, for messages.
    fn who(self) -> String {
        format!(
            "the lens reviewers ({} at depth {}) and the specialists state.json records as dispatched",
            self.level.lens_reviewers(),
            self.level.as_str()
        )
    }
}

/// The parsed files of one review directory; absent files are `None` or empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewDir {
    pub state: State,
    pub problem: Option<Problem>,
    /// `(reviews/<n>.json, review)` in file-name order.
    pub reviews: Vec<(String, Review)>,
    pub investigations: Shards,
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

    /// Every proof git must hold; a kept finding's commit must be on the merged proofs branch.
    /// Once `verified.json` exists only its proofs count: the verifier deletes the
    /// per-reviewer proofs branches, so their commits may be pruned.
    pub fn proofs(&self) -> Vec<Cited<'_>> {
        let unmerged = if self.verified.is_some() {
            &[][..]
        } else {
            &self.reviews[..]
        };
        let reviews = unmerged.iter().flat_map(|(file, review)| {
            review
                .findings
                .iter()
                .enumerate()
                .filter_map(move |(index, finding)| {
                    Some(Cited {
                        at: format!("{file}: findings[{index}].proof"),
                        proof: finding.proof.as_ref()?,
                        branch: None,
                    })
                })
        });
        let merged = proof::merged_branch(&self.state.head);
        let verified = self.verified.iter().flat_map(|verified| {
            verified
                .findings
                .iter()
                .enumerate()
                .filter_map(|(index, finding)| {
                    Some(Cited {
                        at: format!("verified.json: findings[{index}].proof"),
                        proof: finding.proof.as_ref()?,
                        branch: Some(merged.clone()),
                    })
                })
        });
        reviews.chain(verified).collect()
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
        let investigations = self.investigations.iter().map(|(&n, doc)| {
            format!(
                "{}: ok ({} items)\n",
                investigations::file(n),
                doc.items.len()
            )
        });
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

/// Reviewer subagents number 1..=last: the depth's lens reviewers, then the specialists
/// `state.json` records as dispatched; investigators number after them.
pub fn check(
    dir: &ReviewDir,
    rules: &BTreeSet<String>,
    specialists: &[Specialist],
    change: &Change,
    head: &HeadFiles,
    proofs: &ProofObjects,
) -> Vec<Diagnostic> {
    let (dispatched, mut errors) = specialists::dispatched(
        specialists,
        dir.state.specialists.as_ref(),
        !dir.reviews.is_empty(),
    );
    if let Some(problem) = &dir.problem {
        errors.extend(problem::check(
            "problem.json",
            problem,
            dir.state.round,
            head,
        ));
    }
    match &dir.state.depth {
        Some(depth) => {
            errors.extend(numbered(
                dir,
                Roster::new(depth.level, &dispatched),
                rules,
                change,
                head,
            ));
        }
        // Without a depth nobody's number is known, so nothing numbered can be checked.
        None if needs_depth(dir) => {
            errors.push(Diagnostic::new(
                "state.json: depth",
                "missing; record the review depth before dispatching reviewers: `\"depth\": {\"level\": \"light\" | \"focused\" | \"full\", \"why\": \"<one line>\"}`",
            ));
            return errors;
        }
        None => {}
    }
    errors.extend(
        dir.proofs()
            .iter()
            .filter_map(|cited| proof::check(cited, proofs)),
    );
    // Until an investigator writes its file, suspicions are open; `complete` demands them
    // resolved once the verifier ran.
    if !dir.investigations.is_empty() {
        errors.extend(investigations::check(&dir.investigations, &dir.reviews));
    }
    if let Some(verified) = &dir.verified {
        let expected = verified::expected(&dir.reviews, dir.problem.as_ref(), dir.state.round);
        errors.extend(verified::check(
            verified,
            &expected,
            dir.state.round,
            rules,
            head,
        ));
    }
    errors
}

/// Reviewers or investigators ran, or the verifier did without a blocking gate stopping
/// the review first: the coordinator chose a depth before any of them.
fn needs_depth(dir: &ReviewDir) -> bool {
    !dir.reviews.is_empty()
        || !dir.investigations.is_empty()
        || (dir.verified.is_some() && !gate_blocks(dir))
}

/// The gate may stop a review only in round 1: a fix round neither reruns it nor
/// recomputes `blocks`, so a stale `blocks: true` must not excuse missing reviews.
fn gate_blocks(dir: &ReviewDir) -> bool {
    matches!(dir.state.round, Round::First)
        && dir.problem.as_ref().is_some_and(|problem| problem.blocks)
}

/// Every check that depends on who numbers where.
fn numbered(
    dir: &ReviewDir,
    roster: Roster,
    rules: &BTreeSet<String>,
    change: &Change,
    head: &HeadFiles,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let last = roster.last;
    let no_investigator = |n: u32| {
        format!(
            "there is no investigator {n}: reviewers 1..{last} are {}; investigators number from {}",
            roster.who(),
            last + 1
        )
    };
    for (file, review) in &dir.reviews {
        let stem = file
            .strip_prefix("reviews/")
            .and_then(|name| name.strip_suffix(".json"))
            .unwrap_or(file);
        // A specialist reviews only what its prompt covers, which the CLI cannot know, and
        // an investigator only suspicions: neither owes coverage of the change.
        let n = review.reviewer.get();
        let unknown = n > last && !review.is_investigator();
        if unknown {
            errors.push(Diagnostic::new(
                format!("{file}: reviewer"),
                format!(
                    "there is no reviewer {n}: reviewers 1..{last} are {}, and only investigators number after them",
                    roster.who()
                ),
            ));
        }
        if review.is_investigator() {
            if n <= last {
                errors.push(Diagnostic::new(
                    format!("{file}: reviewer"),
                    no_investigator(n),
                ));
            } else if !dir.investigations.contains_key(&review.reviewer) {
                errors.push(Diagnostic::new(
                    file,
                    format!(
                        "investigator {n} wrote reviews/{n}.json but no {}; every investigator resolves its group there",
                        investigations::file(review.reviewer)
                    ),
                ));
            }
        }
        let owes_coverage = !roster.is_specialist(n) && !unknown && !review.is_investigator();
        errors.extend(review::check(
            file,
            stem,
            review,
            owes_coverage,
            rules,
            change,
            head,
        ));
    }
    errors.extend(
        dir.investigations
            .keys()
            .filter(|n| n.get() <= last)
            .map(|&n| Diagnostic::new(investigations::file(n), no_investigator(n.get()))),
    );
    if dir.verified.is_some() {
        errors.extend(complete(dir, roster));
    }
    errors
}

/// What the verifier needs before it runs: every dispatched reviewer's file (unless a blocking gate
/// stopped the review before any ran) and, when there are suspicions, their investigations.
fn complete(dir: &ReviewDir, roster: Roster) -> Vec<Diagnostic> {
    let last = roster.last;
    let present: BTreeSet<String> = dir.reviews.iter().map(|(file, _)| file.clone()).collect();
    let required: Vec<String> = (1..=last).map(|n| format!("reviews/{n}.json")).collect();
    let stopped_at_gate = gate_blocks(dir) && required.iter().all(|file| !present.contains(file));
    let missing_reviews = required
        .into_iter()
        .filter(|file| !stopped_at_gate && !present.contains(file))
        .map(|file| {
            Diagnostic::new(
                format!("{file} missing"),
                format!(
                    "{} need reviews/1.json..{last}.json, one subagent each",
                    roster.who()
                ),
            )
        });
    let suspicions: Vec<String> = investigations::raised(&dir.reviews)
        .keys()
        .map(ToString::to_string)
        .collect();
    let missing_investigations =
        (dir.investigations.is_empty() && !suspicions.is_empty()).then(|| {
            Diagnostic::new(
                investigations::DIR,
                format!(
                    "missing; reviews hold suspicions ({}) that investigators must resolve in {}/<n>.json, n from {}",
                    suspicions.join(", "),
                    investigations::DIR,
                    last + 1
                ),
            )
        });
    missing_reviews.chain(missing_investigations).collect()
}
