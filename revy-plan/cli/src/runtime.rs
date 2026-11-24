//! The runtime check layer.
//!
//! The synchronous ingest boundary where an external result's filesystem-backed
//! facts are verified before it becomes a [`CheckedWorkResult`]: evidence source
//! anchors and runtime-artifact transcript digests, plus the approved-tool-usage
//! obligation (design decision 6). The plan is only read; a rejected result leaves
//! it untouched, and the target is never written. No probe execution, no
//! verification runs.

use std::path::Path;

use thiserror::Error;

use crate::document::ParsedPlan;
use crate::evidence::{Evidence, SemanticObservation, StructuralObservation, TranscriptRef};
use crate::ids::{EvidenceId, ProposalId};
use crate::packet::{CheckedWorkResult, ExplorationResult, RawWorkResult};
use crate::primitives::{Location, NonEmptyText, RepoRelPath, Sha256Digest};
use crate::state::RunState;

/// A filesystem-backed or approved-tool-usage fact a result asserts that does not
/// hold at ingest.
#[derive(Debug, Error)]
pub enum CheckError {
    #[error("evidence `{evidence}` names source anchor `{path}` that is not present: {detail}")]
    MissingSourceAnchor {
        evidence: EvidenceId,
        path: String,
        detail: String,
    },
    #[error("evidence `{evidence}` names line {line} in `{path}`, past the end of the file")]
    AnchorLineOutOfRange {
        evidence: EvidenceId,
        path: String,
        line: u32,
    },
    #[error("evidence `{evidence}` artifact digest does not match its contents")]
    ExplorationTranscriptDigestMismatch { evidence: EvidenceId },
    #[error(
        "approved tool `{proposal}` is not used: no exploration evidence carries it as provenance"
    )]
    ApprovedToolUnused { proposal: ProposalId },
}

/// The two independent roots a runtime check resolves paths against: the immutable
/// `target` checkout for source anchors, and the plan's external `transcripts`
/// home for runtime-artifact digests.
#[derive(Debug, Clone, Copy)]
struct CheckRoots<'a> {
    target: &'a Path,
    transcripts: &'a Path,
}

/// Check a raw external result's filesystem-backed facts, promoting it to a
/// [`CheckedWorkResult`] or failing loud.
pub fn check_result(
    plan: &ParsedPlan,
    transcripts_root: &Path,
    result: RawWorkResult,
) -> Result<CheckedWorkResult, CheckError> {
    let roots = CheckRoots {
        target: Path::new(plan.repository().root.as_str()),
        transcripts: transcripts_root,
    };
    match &result {
        RawWorkResult::Exploration(exploration) => {
            check_exploration(roots, exploration)?;
            check_approved_tools(plan, exploration)?;
        }
        // A correction carries a full revised candidate whose exploration evidence
        // has its source anchors validated exactly as a first exploration's are.
        RawWorkResult::Correction(correction) => {
            for evidence in &correction.candidate.exploration.evidence {
                check_evidence_anchors(roots, evidence)?;
            }
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
    Ok(CheckedWorkResult::checked(result))
}

/// Prove every approved tool appears as evidence provenance in this exploration
/// (design decision 6). An installed-but-unused tool fails exploration.
fn check_approved_tools(plan: &ParsedPlan, result: &ExplorationResult) -> Result<(), CheckError> {
    let RunState::Exploring { capabilities, .. } = plan.state() else {
        return Ok(());
    };
    let tools_used: std::collections::BTreeSet<&str> = result
        .evidence
        .iter()
        .filter_map(evidence_tool)
        .map(NonEmptyText::as_str)
        .collect();
    for proposal in capabilities.approved() {
        let name = capabilities
            .report()
            .proposals
            .iter()
            .find(|candidate| &candidate.id == proposal)
            .map(|candidate| candidate.name.as_str());
        if let Some(name) = name
            && !tools_used.contains(name)
        {
            return Err(CheckError::ApprovedToolUnused {
                proposal: proposal.clone(),
            });
        }
    }
    Ok(())
}

/// The tool an evidence item was gathered with, if it records provenance
/// (semantic and structural observations do).
const fn evidence_tool(evidence: &Evidence) -> Option<&NonEmptyText> {
    match evidence {
        Evidence::Semantic(evidence) => Some(&evidence.provenance.tool),
        Evidence::Structural(evidence) => Some(&evidence.provenance.tool),
        Evidence::Runtime(_)
        | Evidence::Textual(_)
        | Evidence::Documentation(_)
        | Evidence::History(_)
        | Evidence::Inference(_) => None,
    }
}

/// Verify every source anchor and artifact digest an exploration result's
/// evidence names against the real repository.
fn check_exploration(roots: CheckRoots<'_>, result: &ExplorationResult) -> Result<(), CheckError> {
    for evidence in &result.evidence {
        check_evidence_anchors(roots, evidence)?;
    }
    Ok(())
}

/// Verify every filesystem anchor and artifact digest one evidence item names.
fn check_evidence_anchors(roots: CheckRoots<'_>, evidence: &Evidence) -> Result<(), CheckError> {
    match evidence {
        Evidence::Runtime(runtime) => {
            verify_artifact(roots.transcripts, &runtime.id, &runtime.artifact)
        }
        Evidence::Textual(textual) => {
            for path in &textual.scope {
                verify_scope(roots.target, &textual.id, path)?;
            }
            for occurrence in &textual.matches {
                verify_anchor(roots.target, &textual.id, &occurrence.location)?;
            }
            Ok(())
        }
        Evidence::Semantic(semantic) => {
            check_semantic_anchors(roots.target, &semantic.id, &semantic.observation)
        }
        Evidence::Structural(structural) => {
            check_structural_anchors(roots.target, &structural.id, &structural.observation)
        }
        Evidence::Documentation(_) | Evidence::History(_) | Evidence::Inference(_) => Ok(()),
    }
}

fn check_semantic_anchors(
    root: &Path,
    evidence: &EvidenceId,
    observation: &SemanticObservation,
) -> Result<(), CheckError> {
    match observation {
        SemanticObservation::Definitions { locations } => {
            for location in locations {
                verify_anchor(root, evidence, location)?;
            }
        }
        SemanticObservation::References { references, .. } => {
            for reference in references {
                verify_anchor(root, evidence, &reference.location)?;
            }
        }
        SemanticObservation::Implementations { locations, .. } => {
            for location in locations {
                verify_anchor(root, evidence, location)?;
            }
        }
        SemanticObservation::CallEdges { edges, .. } => {
            for edge in edges {
                verify_anchor(root, evidence, &edge.caller)?;
                verify_anchor(root, evidence, &edge.callee)?;
            }
        }
    }
    Ok(())
}

fn check_structural_anchors(
    root: &Path,
    evidence: &EvidenceId,
    observation: &StructuralObservation,
) -> Result<(), CheckError> {
    match observation {
        StructuralObservation::Symbols { locations, .. } => {
            for location in locations {
                verify_anchor(root, evidence, location)?;
            }
        }
        StructuralObservation::Imports { .. } => {}
    }
    Ok(())
}

/// Verify a textual-evidence scope path is present. A scope names where a search
/// ran, so it may resolve to a file or a directory — an absence claim (no matches)
/// can legitimately scope a whole directory. Existence is the only anchor here;
/// line ranges are checked against the files a match actually anchors to
/// ([`verify_anchor`]), never against a scope, so a directory scope is not opened
/// as a file.
fn verify_scope(root: &Path, evidence: &EvidenceId, path: &RepoRelPath) -> Result<(), CheckError> {
    let resolved = root.join(path.as_str());
    match resolved.try_exists() {
        Ok(true) => Ok(()),
        Ok(false) => Err(CheckError::MissingSourceAnchor {
            evidence: evidence.clone(),
            path: path.as_str().to_owned(),
            detail: format!("no such path; looked for {}", resolved.display()),
        }),
        Err(error) => Err(CheckError::MissingSourceAnchor {
            evidence: evidence.clone(),
            path: path.as_str().to_owned(),
            detail: format!("{error}; looked for {}", resolved.display()),
        }),
    }
}

/// Read a source file the evidence names, failing loud if it is not present.
fn verify_source(
    root: &Path,
    evidence: &EvidenceId,
    path: &RepoRelPath,
) -> Result<Vec<u8>, CheckError> {
    let resolved = root.join(path.as_str());
    std::fs::read(&resolved).map_err(|error| CheckError::MissingSourceAnchor {
        evidence: evidence.clone(),
        path: path.as_str().to_owned(),
        detail: format!("{error}; looked for {}", resolved.display()),
    })
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
            path: location.path.as_str().to_owned(),
            line: location.line,
        });
    }
    Ok(())
}

/// Verify an evidence artifact's digest against the captured file on disk.
fn verify_artifact(
    root: &Path,
    evidence: &EvidenceId,
    artifact: &TranscriptRef,
) -> Result<(), CheckError> {
    let contents = verify_source(root, evidence, &artifact.path)?;
    if Sha256Digest::hash(&contents) == artifact.digest {
        Ok(())
    } else {
        Err(CheckError::ExplorationTranscriptDigestMismatch {
            evidence: evidence.clone(),
        })
    }
}
