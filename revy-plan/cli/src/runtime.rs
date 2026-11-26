//! The runtime check layer.
//!
//! The synchronous ingest boundary where an external result's filesystem-backed
//! facts are verified before it becomes a [`CheckedWorkResult`]: evidence source
//! anchors and runtime-artifact transcript digests, plus the approved-tool-usage
//! obligation (design decision 6). Evidence that omits its repository identity is
//! stamped with the plan's identity when the live checkout still matches it. The
//! plan is only read; a rejected result leaves it untouched, and the target is
//! never written. No probe execution, no verification runs. Every independent
//! defect is reported, each at a JSON pointer into the result.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use thiserror::Error;

use crate::capability::RepositoryIdentity;
use crate::command::CommandSpec;
use crate::document::ParsedPlan;
use crate::evidence::{Evidence, TranscriptRef};
use crate::ids::{EvidenceId, ProposalId};
use crate::issues::{Collector, Issues, JsonPointer};
use crate::packet::{
    CheckedRevision, CheckedWorkResult, ExplorationResult, ProvisionalRevision,
    ProvisionalWorkResult, RawWorkResult, RevisionResult,
};
use crate::primitives::{
    Location, NonEmptyText, RepoAlias, RepoPath, RepoScope, ScopeExtent, Sha256Digest,
};
use crate::repository::{Repo, Repositories, Repository};
use crate::state::{CitedRepo, RunState, cited_repo, evidence_id};

/// A filesystem-backed, repository-identity, or approved-tool-usage fact a result
/// asserts that does not hold at ingest.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CheckError {
    #[error(
        "evidence `{evidence}` names source anchor `{path}` that is not present under target root `{root}`: {detail}"
    )]
    MissingSourceAnchor {
        evidence: EvidenceId,
        path: String,
        root: String,
        detail: String,
    },
    #[error(
        "evidence `{evidence}` names line {line} in `{path}` (under target root `{root}`), outside the file's lines 1..={lines}"
    )]
    AnchorLineOutOfRange {
        evidence: EvidenceId,
        path: String,
        root: String,
        line: u32,
        lines: usize,
    },
    #[error(
        "evidence `{evidence}` artifact `{path}` does not verify against its declared digest `{declared}`: {plan_lookup}; {shared_lookup}; run `revy-plan transcript add FILE` and use the reference it prints"
    )]
    ExplorationTranscriptDigestMismatch {
        evidence: EvidenceId,
        path: String,
        declared: Sha256Digest,
        plan_lookup: String,
        shared_lookup: String,
    },
    #[error(
        "approved tool `{proposal}` (\"{name}\") is unused; provenance seen: {seen:?}; set provenance.tool (or a runtime command's program) to one of {accepted:?} on the evidence it produced, or record a `tool_failures` entry {{\"proposal\": \"{proposal}\", \"reason\": ...}}"
    )]
    ApprovedToolUnused {
        proposal: ProposalId,
        name: NonEmptyText,
        seen: Vec<String>,
        accepted: Vec<String>,
    },
    #[error(
        "evidence `{evidence}` artifact `{path}` (digest `{digest}`) is in neither the plan's transcripts `{plan_dir}` nor the shared store `{shared_dir}`; run `revy-plan transcript add FILE` and use the reference it prints"
    )]
    TranscriptNotFound {
        evidence: EvidenceId,
        path: String,
        digest: Sha256Digest,
        plan_dir: String,
        shared_dir: String,
    },
    #[error(
        "evidence {} omits `target`, but the checkout of repository `{repo}` (`{root}`) moved since the plan was captured: plan revision `{}` (dirty digest `{}`), checkout now at revision `{}` (dirty digest `{}`); restore the checkout, or run `revy-plan reopen exploration --recapture --reason TEXT` to re-capture it and re-verify the carried evidence against it",
        backticked(evidence), plan.revision, plan.dirty_digest, checkout.revision, checkout.dirty_digest
    )]
    CheckoutMoved {
        evidence: Vec<EvidenceId>,
        repo: RepoAlias,
        root: String,
        plan: Box<RepositoryIdentity>,
        checkout: Box<RepositoryIdentity>,
    },
    #[error(
        "evidence {} omits `target`, and the checkout of repository `{repo}` could not be captured to confirm it still matches the plan: {detail}",
        backticked(evidence)
    )]
    CheckoutUnavailable {
        evidence: Vec<EvidenceId>,
        repo: RepoAlias,
        detail: String,
    },
}

impl CheckError {
    /// The evidence this defect left unstamped because its checkout moved or
    /// could not be captured — the grounding defect those items then also
    /// have is this one, not a second problem.
    pub fn unstamped_evidence(&self) -> &[EvidenceId] {
        match self {
            Self::CheckoutMoved { evidence, .. } | Self::CheckoutUnavailable { evidence, .. } => {
                evidence
            }
            Self::MissingSourceAnchor { .. }
            | Self::AnchorLineOutOfRange { .. }
            | Self::ExplorationTranscriptDigestMismatch { .. }
            | Self::ApprovedToolUnused { .. }
            | Self::TranscriptNotFound { .. } => &[],
        }
    }
}

fn backticked(ids: &[EvidenceId]) -> String {
    ids.iter()
        .map(|id| format!("`{id}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The live state of one of the plan's checkouts, captured by the caller at
/// ingest. Only consulted when evidence omits its `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checkout {
    Captured(RepositoryIdentity),
    Unavailable { detail: String },
}

impl Checkout {
    /// Capture the checkout rooted at `repository`'s root.
    pub fn capture(repository: &Repository, now: &str) -> Self {
        match crate::repository::capture(Path::new(repository.root.as_str()), now) {
            Ok(captured) => Self::Captured(captured.identity()),
            Err(error) => Self::Unavailable {
                detail: error.to_string(),
            },
        }
    }
}

/// The live state of every one of the plan's checkouts: the primary, and each
/// declared repository by alias. A repository with no entry was not captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkouts {
    primary: Checkout,
    others: BTreeMap<RepoAlias, Checkout>,
}

impl Checkouts {
    /// Only the primary checkout; any other repository counts as uncaptured.
    pub const fn primary(primary: Checkout) -> Self {
        Self {
            primary,
            others: BTreeMap::new(),
        }
    }

    /// The primary checkout plus each declared repository's.
    pub const fn new(primary: Checkout, others: BTreeMap<RepoAlias, Checkout>) -> Self {
        Self { primary, others }
    }

    /// Capture every repository the plan declares.
    pub fn capture(repositories: &Repositories, now: &str) -> Self {
        Self {
            primary: Checkout::capture(repositories.primary(), now),
            others: repositories
                .others()
                .iter()
                .map(|(alias, repository)| (alias.clone(), Checkout::capture(repository, now)))
                .collect(),
        }
    }

    fn of(&self, repo: Repo<'_>) -> Checkout {
        let found = if repo.primary {
            Some(&self.primary)
        } else {
            self.others.get(repo.alias)
        };
        found.cloned().unwrap_or_else(|| Checkout::Unavailable {
            detail: format!("repository `{}` was not captured", repo.alias),
        })
    }
}

/// A result that failed its runtime check: every defect, plus the result itself
/// kept provisional so its transition can be rehearsed for further defects.
#[derive(Debug)]
pub struct RuntimeRejection<P = ProvisionalWorkResult> {
    issues: Issues<CheckError>,
    provisional: Box<P>,
}

impl<P> RuntimeRejection<P> {
    pub const fn issues(&self) -> &Issues<CheckError> {
        &self.issues
    }

    pub fn into_parts(self) -> (Issues<CheckError>, P) {
        (self.issues, *self.provisional)
    }
}

impl<P> std::fmt::Display for RuntimeRejection<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.issues)
    }
}

impl<P: std::fmt::Debug> std::error::Error for RuntimeRejection<P> {}

/// Where runtime-artifact transcripts are looked up.
///
/// The plan's own `transcripts/` directory (by the reference's path) first,
/// then the shared content-addressed store `$REVY_PLAN_HOME/transcripts/<sha256>`
/// (by digest).
#[derive(Debug, Clone, Copy)]
pub struct TranscriptStores<'a> {
    pub plan: &'a Path,
    pub shared: &'a Path,
}

/// The independent roots a runtime check resolves paths against: each
/// repository's immutable checkout for the source anchors citing it, and the
/// transcript stores for runtime-artifact digests.
#[derive(Debug, Clone, Copy)]
struct CheckRoots<'a> {
    repositories: &'a Repositories,
    transcripts: TranscriptStores<'a>,
}

impl CheckRoots<'_> {
    /// The checkout root a citation names, or `None` when its alias does not
    /// resolve — grounding reports that, once, with the declared aliases.
    fn root(&self, repo: Option<&RepoAlias>) -> Option<&Path> {
        self.repositories
            .resolve(repo)
            .ok()
            .map(|repo| Path::new(repo.repository.root.as_str()))
    }
}

/// Check a raw external result's filesystem-backed facts, stamping evidence
/// that omits its repository identity, and promoting it to a
/// [`CheckedWorkResult`] or reporting every defect.
pub fn check_result(
    plan: &ParsedPlan,
    transcripts: TranscriptStores<'_>,
    checkouts: &Checkouts,
    mut result: RawWorkResult,
) -> Result<CheckedWorkResult, RuntimeRejection> {
    let roots = CheckRoots {
        repositories: plan.repositories(),
        transcripts,
    };
    let mut issues = Collector::new();
    match &mut result {
        RawWorkResult::Exploration(exploration) => {
            let base = JsonPointer::root();
            stamp_targets(
                plan.repositories(),
                checkouts,
                &mut exploration.evidence,
                &base,
                &mut issues,
            );
            check_anchors(roots, &exploration.evidence, &base, &mut issues);
            check_approved_tools(plan, exploration, &mut issues);
        }
        // A correction carries a full revised candidate whose exploration evidence
        // is stamped and anchor-checked exactly as a first exploration's is.
        RawWorkResult::Correction(correction) => {
            let base = JsonPointer::new("/candidate/exploration");
            let evidence = &mut correction.candidate.exploration.evidence;
            stamp_targets(plan.repositories(), checkouts, evidence, &base, &mut issues);
            check_anchors(roots, evidence, &base, &mut issues);
        }
        // The remaining results carry no filesystem-backed facts; their proofs run
        // in the workflow transition, not here.
        RawWorkResult::CapabilityReport(_)
        | RawWorkResult::CapabilityDecision(_)
        | RawWorkResult::DesignBrief(_)
        | RawWorkResult::UserDecision(_)
        | RawWorkResult::Draft(_)
        | RawWorkResult::Review(_)
        | RawWorkResult::ReviewDecision(_) => {}
    }
    match issues.finish(()) {
        Ok(()) => Ok(CheckedWorkResult::checked(result)),
        Err(issues) => Err(RuntimeRejection {
            issues,
            provisional: Box::new(ProvisionalWorkResult::provisional(result)),
        }),
    }
}

/// Check a revision's candidate evidence exactly as a correction's: stamp
/// omitted targets, verify anchors and transcripts.
pub fn check_revision(
    plan: &ParsedPlan,
    transcripts: TranscriptStores<'_>,
    checkouts: &Checkouts,
    mut revision: RevisionResult,
) -> Result<CheckedRevision, RuntimeRejection<ProvisionalRevision>> {
    let roots = CheckRoots {
        repositories: plan.repositories(),
        transcripts,
    };
    let mut issues = Collector::new();
    let base = JsonPointer::new("/candidate/exploration");
    let evidence = &mut revision.candidate.exploration.evidence;
    stamp_targets(plan.repositories(), checkouts, evidence, &base, &mut issues);
    check_anchors(roots, evidence, &base, &mut issues);
    match issues.finish(()) {
        Ok(()) => Ok(CheckedRevision::checked(revision)),
        Err(issues) => Err(RuntimeRejection {
            issues,
            provisional: Box::new(ProvisionalRevision::provisional(revision)),
        }),
    }
}

/// The `target` slot of evidence kinds whose identity is optional on input,
/// when it is empty.
fn missing_target(evidence: &mut Evidence) -> Option<&mut Option<RepositoryIdentity>> {
    evidence.optional_target_mut().filter(|slot| slot.is_none())
}

/// Fill every omitted `target` with the identity of the repository the
/// evidence cites (the primary when it cites none) — but only when that live
/// checkout still is that identity, so stamping never vouches for an
/// observation taken against a moved checkout. Evidence citing no single
/// declared repository is left for grounding to report.
fn stamp_targets(
    repositories: &Repositories,
    checkouts: &Checkouts,
    evidence: &mut [Evidence],
    base: &JsonPointer,
    issues: &mut Collector<CheckError>,
) {
    let mut unstamped: BTreeMap<&RepoAlias, (Repo<'_>, Vec<usize>)> = BTreeMap::new();
    for (position, item) in evidence.iter_mut().enumerate() {
        if missing_target(item).is_none() {
            continue;
        }
        // Anchorless evidence names no repository; only a single-repository
        // plan lets that mean the primary. Grounding reports the rest.
        let repo = match cited_repo(item, repositories) {
            Ok(CitedRepo::One(repo)) => repo,
            Ok(CitedRepo::Anchorless) if repositories.others().is_empty() => {
                repositories.primary_repo()
            }
            Ok(CitedRepo::Anchorless) | Err(_) => continue,
        };
        unstamped
            .entry(repo.alias)
            .or_insert_with(|| (repo, Vec::new()))
            .1
            .push(position);
    }
    for (repo, positions) in unstamped.into_values() {
        let plan = repo.repository.identity();
        let ids: Vec<EvidenceId> = positions
            .iter()
            .filter_map(|position| evidence.get(*position))
            .map(|item| evidence_id(item).clone())
            .collect();
        match checkouts.of(repo) {
            Checkout::Captured(live) if live == plan => {
                for position in &positions {
                    if let Some(slot) = evidence.get_mut(*position).and_then(missing_target) {
                        *slot = Some(plan.clone());
                    }
                }
            }
            Checkout::Captured(live) => issues.push(
                base.join("evidence"),
                CheckError::CheckoutMoved {
                    evidence: ids,
                    repo: repo.alias.clone(),
                    root: repo.repository.root.to_string(),
                    plan: Box::new(plan),
                    checkout: Box::new(live),
                },
            ),
            Checkout::Unavailable { detail } => issues.push(
                base.join("evidence"),
                CheckError::CheckoutUnavailable {
                    evidence: ids,
                    repo: repo.alias.clone(),
                    detail,
                },
            ),
        }
    }
}

/// Every name an approved proposal may appear under as provenance: its id and
/// its tool name, matched case-sensitively.
fn accepted_names(proposal: &ProposalId, name: &NonEmptyText) -> Vec<String> {
    vec![proposal.as_str().to_owned(), name.as_str().to_owned()]
}

/// Prove every approved tool appears as evidence provenance (or a runtime
/// command's program) in this exploration, or is recorded as a tool failure
/// (design decision 6). An installed-but-unused tool fails exploration.
fn check_approved_tools(
    plan: &ParsedPlan,
    result: &ExplorationResult,
    issues: &mut Collector<CheckError>,
) {
    let RunState::Exploring { capabilities, .. } = plan.state() else {
        return;
    };
    let seen: BTreeSet<&str> = result.evidence.iter().filter_map(evidence_tool).collect();
    let failed: BTreeSet<&ProposalId> = result
        .tool_failures
        .iter()
        .map(|failure| &failure.proposal)
        .collect();
    for proposal in capabilities.approved() {
        let Some(disclosed) = capabilities
            .report()
            .proposals
            .iter()
            .find(|candidate| &candidate.id == proposal)
        else {
            continue;
        };
        let accepted = accepted_names(proposal, &disclosed.name);
        let used = accepted.iter().any(|name| seen.contains(name.as_str()));
        if !used && !failed.contains(proposal) {
            issues.push(
                JsonPointer::new("/evidence"),
                CheckError::ApprovedToolUnused {
                    proposal: proposal.clone(),
                    name: disclosed.name.clone(),
                    seen: seen.iter().map(|tool| (*tool).to_owned()).collect(),
                    accepted,
                },
            );
        }
    }
}

/// The tool an evidence item was gathered with, if it records one: semantic and
/// structural provenance, or a runtime command's program.
fn evidence_tool(evidence: &Evidence) -> Option<&str> {
    match evidence {
        Evidence::Semantic(evidence) => Some(evidence.provenance.tool.as_str()),
        Evidence::Structural(evidence) => Some(evidence.provenance.tool.as_str()),
        Evidence::Runtime(evidence) => match &evidence.command {
            CommandSpec::Program { program, .. } => Some(program.as_str()),
            CommandSpec::Shell { .. } => None,
        },
        Evidence::Textual(_)
        | Evidence::Documentation(_)
        | Evidence::History(_)
        | Evidence::Inference(_) => None,
    }
}

/// Verify every source anchor and artifact digest the evidence names.
fn check_anchors(
    roots: CheckRoots<'_>,
    evidence: &[Evidence],
    base: &JsonPointer,
    issues: &mut Collector<CheckError>,
) {
    for (position, item) in evidence.iter().enumerate() {
        let pointer = base.join("evidence").join(position);
        for error in evidence_anchor_errors(roots, item) {
            issues.push(pointer.clone(), error);
        }
    }
}

/// Every filesystem anchor and artifact digest defect one evidence item has.
fn evidence_anchor_errors(roots: CheckRoots<'_>, evidence: &Evidence) -> Vec<CheckError> {
    let id = evidence_id(evidence);
    let scopes: Vec<&RepoScope> = match evidence {
        Evidence::Runtime(runtime) => {
            return verify_artifact(roots.transcripts, &runtime.id, &runtime.artifact)
                .err()
                .into_iter()
                .collect();
        }
        Evidence::Textual(textual) => textual.scope.iter().collect(),
        Evidence::Semantic(_)
        | Evidence::Structural(_)
        | Evidence::Documentation(_)
        | Evidence::History(_)
        | Evidence::Inference(_) => Vec::new(),
    };
    let scope_errors = scopes.into_iter().filter_map(|scope| {
        let root = roots.root(scope.repo())?;
        verify_scope(root, id, scope).err()
    });
    let anchor_errors = evidence.locations().into_iter().filter_map(|location| {
        let root = roots.root(location.path.repo())?;
        verify_anchor(root, id, location).err()
    });
    scope_errors.chain(anchor_errors).collect()
}

fn missing_anchor(
    root: &Path,
    evidence: &EvidenceId,
    path: &impl std::fmt::Display,
    detail: String,
) -> CheckError {
    CheckError::MissingSourceAnchor {
        evidence: evidence.clone(),
        path: path.to_string(),
        root: root.display().to_string(),
        detail,
    }
}

/// Verify a textual-evidence scope path is present. A scope names where a search
/// ran, so it may resolve to a file or a directory — an absence claim (no matches)
/// can legitimately scope a whole directory. Existence is the only anchor here;
/// line ranges are checked against the files a match actually anchors to
/// ([`verify_anchor`]), never against a scope, so a directory scope is not opened
/// as a file.
fn verify_scope(root: &Path, evidence: &EvidenceId, scope: &RepoScope) -> Result<(), CheckError> {
    let path = match scope.extent() {
        ScopeExtent::WholeRepository => root.to_path_buf(),
        ScopeExtent::Path(path) => root.join(path.as_str()),
    };
    match path.try_exists() {
        Ok(true) => Ok(()),
        Ok(false) => Err(missing_anchor(
            root,
            evidence,
            scope,
            "no such path".to_owned(),
        )),
        Err(error) => Err(missing_anchor(root, evidence, scope, error.to_string())),
    }
}

/// Read a file the evidence names, failing loud if it is not present.
fn verify_source(
    root: &Path,
    evidence: &EvidenceId,
    path: &RepoPath,
) -> Result<Vec<u8>, CheckError> {
    std::fs::read(root.join(path.path().as_str()))
        .map_err(|error| missing_anchor(root, evidence, path, error.to_string()))
}

/// Verify a location anchor: its file exists and its line is within the file.
fn verify_anchor(
    root: &Path,
    evidence: &EvidenceId,
    location: &Location,
) -> Result<(), CheckError> {
    let contents = verify_source(root, evidence, &location.path)?;
    let lines = String::from_utf8_lossy(&contents).lines().count();
    let line = usize::try_from(location.line).unwrap_or(usize::MAX);
    if location.line == 0 || line > lines {
        return Err(CheckError::AnchorLineOutOfRange {
            evidence: evidence.clone(),
            path: location.path.to_string(),
            root: root.display().to_string(),
            line: location.line,
            lines,
        });
    }
    Ok(())
}

/// What one transcript store holds for a reference.
enum Lookup {
    Verified,
    Absent,
    Mismatch(Sha256Digest),
    Unreadable(String),
}

fn look_up(file: &Path, declared: &Sha256Digest) -> Lookup {
    match std::fs::read(file) {
        Ok(contents) => {
            let actual = Sha256Digest::hash(&contents);
            if &actual == declared {
                Lookup::Verified
            } else {
                Lookup::Mismatch(actual)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Lookup::Absent,
        Err(error) => Lookup::Unreadable(error.to_string()),
    }
}

/// Verify an evidence artifact's digest: against the plan's own copy at its
/// path, and — when that copy is absent or does not match — against the
/// shared store's copy named by the digest.
fn verify_artifact(
    stores: TranscriptStores<'_>,
    evidence: &EvidenceId,
    artifact: &TranscriptRef,
) -> Result<(), CheckError> {
    let local = stores.plan.join(artifact.path.as_str());
    let plan_lookup = look_up(&local, &artifact.digest);
    let shared = stores.shared.join(artifact.digest.as_str());
    let shared_lookup = look_up(&shared, &artifact.digest);
    let plan_text = match plan_lookup {
        Lookup::Verified => return Ok(()),
        Lookup::Absent => match shared_lookup {
            Lookup::Verified => return Ok(()),
            Lookup::Absent => {
                return Err(CheckError::TranscriptNotFound {
                    evidence: evidence.clone(),
                    path: artifact.path.as_str().to_owned(),
                    digest: artifact.digest.clone(),
                    plan_dir: stores.plan.display().to_string(),
                    shared_dir: stores.shared.display().to_string(),
                });
            }
            Lookup::Mismatch(_) | Lookup::Unreadable(_) => format!(
                "the plan's transcripts `{}` have no `{}`",
                stores.plan.display(),
                artifact.path
            ),
        },
        Lookup::Mismatch(actual) => {
            format!("the plan's copy `{}` hashes to `{actual}`", local.display())
        }
        Lookup::Unreadable(detail) => {
            format!(
                "the plan's copy `{}` is unreadable: {detail}",
                local.display()
            )
        }
    };
    let shared_text = match shared_lookup {
        Lookup::Verified => return Ok(()),
        Lookup::Absent => format!(
            "the shared store `{}` has no `{}`",
            stores.shared.display(),
            artifact.digest
        ),
        Lookup::Mismatch(actual) => format!(
            "the shared store's copy `{}` hashes to `{actual}`",
            shared.display()
        ),
        Lookup::Unreadable(detail) => format!(
            "the shared store's copy `{}` is unreadable: {detail}",
            shared.display()
        ),
    };
    Err(CheckError::ExplorationTranscriptDigestMismatch {
        evidence: evidence.clone(),
        path: artifact.path.as_str().to_owned(),
        declared: artifact.digest.clone(),
        plan_lookup: plan_text,
        shared_lookup: shared_text,
    })
}
