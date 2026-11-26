//! The proof-carrying plan document: raw bytes are parsed once into a
//! [`ParsedPlan`] whose every cross-reference has been resolved.
//!
//! The raw [`RawPlanDocument`] is the sole schema/deserialization source; proven
//! state is never deserialized directly.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use schemars::{JsonSchema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_path_to_error::Segment;
use thiserror::Error;

use crate::capability::CapabilityReport;
use crate::evidence::Evidence;
use crate::ids::{ClaimId, EvidenceId};
use crate::issues::Issues;
use crate::model::delivery::DeliveryPlan;
use crate::model::impact::ImpactEntry;
use crate::model::review::{PlanRegion, RegionFingerprint, fingerprints};
use crate::primitives::{NonEmptyText, RepoAlias, Slug};
use crate::repository::{DeclarationError, Repositories, Repository};
use crate::state::{Candidate, RawExplorationContext, RawRunState, RunState, history_finding_ids};

/// The raw, deserializable plan document. This is the one and only full-plan
/// schema source; there is no second schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawPlanDocument {
    pub slug: Slug,
    /// The primary repository: it keys the plan home and is cited unprefixed.
    pub repository: Repository,
    /// The primary repository's alias; omitted when it is the default derived
    /// from `repository.name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_alias: Option<RepoAlias>,
    /// Every other repository the plan cites, by alias (`alias:path`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub repositories: BTreeMap<RepoAlias, Repository>,
    pub run: RawRunState,
}

impl RawPlanDocument {
    /// The proven repository set this document declares.
    pub fn declared_repositories(&self) -> Result<Repositories, DeclarationError> {
        let alias = self
            .repository_alias
            .clone()
            .unwrap_or_else(|| RepoAlias::for_key(&self.repository.name));
        Repositories::new(
            alias,
            self.repository.clone(),
            self.repositories
                .iter()
                .map(|(alias, repository)| (alias.clone(), repository.clone())),
        )
    }
}

/// Where an evidence item was found in the parsed document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceIndex {
    pointer: String,
}

impl EvidenceIndex {
    pub fn pointer(&self) -> &str {
        &self.pointer
    }
}

/// Where a claim was found in the parsed document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimIndex {
    pointer: String,
}

impl ClaimIndex {
    pub fn pointer(&self) -> &str {
        &self.pointer
    }
}

/// A parse failure located by a JSON pointer into the source document.
#[derive(Debug, Error)]
#[error("{message} (at {pointer})")]
pub struct ParsePlanError {
    pointer: String,
    message: String,
}

impl ParsePlanError {
    /// The JSON pointer to the offending location in the source document.
    pub fn pointer(&self) -> &str {
        &self.pointer
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    const fn at(pointer: String, message: String) -> Self {
        Self { pointer, message }
    }

    /// The first defect's pointer (re-rooted under `base`), with every defect
    /// listed in the message.
    fn from_issues<E: std::fmt::Display>(base: &str, issues: &Issues<E>) -> Self {
        let first = issues.first();
        let mut message = first.error().to_string();
        for located in issues.iter().skip(1) {
            let _ = write!(
                message,
                "; {} (at {base}{})",
                located.error(),
                located.pointer().as_str()
            );
        }
        Self::at(format!("{base}{}", first.pointer().as_str()), message)
    }

    fn from_serde(error: &serde_path_to_error::Error<serde_json::Error>) -> Self {
        let message = error.inner().to_string();
        let base = json_pointer_from_path(error.path());
        // `RawRunState` is adjacently tagged (`content = "data"`), so serde does
        // not buffer the variant body: `serde_path_to_error` sees inside it and a
        // missing field is reported against the exact `/run/data/...` container, so
        // appending the field name is always correct.
        let pointer = extract_backtick_field(&message, "missing field")
            .map_or_else(|| base.clone(), |field| append_segment(&base, &field));
        Self { pointer, message }
    }
}

/// A plan document whose run state, evidence, claims, and cross-references have
/// all been proven. Constructed only by [`ParsedPlan::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPlan {
    slug: Slug,
    repositories: Repositories,
    state: RunState,
    evidence_by_id: BTreeMap<EvidenceId, EvidenceIndex>,
    claim_by_id: BTreeMap<ClaimId, ClaimIndex>,
    region_fingerprints: BTreeMap<PlanRegion, RegionFingerprint>,
}

impl ParsedPlan {
    /// A fresh plan awaiting the user's decision over its capability report —
    /// or already exploring when the report leaves nothing to decide.
    #[must_use]
    pub fn new(slug: Slug, repositories: Repositories, report: CapabilityReport) -> Self {
        Self {
            slug,
            repositories,
            state: RunState::opening(report),
            evidence_by_id: BTreeMap::new(),
            claim_by_id: BTreeMap::new(),
            region_fingerprints: BTreeMap::new(),
        }
    }

    /// Parse raw bytes into a proven plan.
    pub fn parse(raw: &[u8]) -> Result<Self, ParsePlanError> {
        let document = deserialize(raw)?;
        let repositories = document
            .declared_repositories()
            .map_err(|error| ParsePlanError::at("/repositories".to_owned(), error.to_string()))?;
        let (evidence_by_id, claim_by_id) = index_and_resolve(&document)?;
        prove_candidate(&document, &repositories)?;
        let region_fingerprints = region_fingerprints(&document);
        let RawPlanDocument { slug, run, .. } = document;
        Ok(Self {
            slug,
            repositories,
            state: run.prove(),
            evidence_by_id,
            claim_by_id,
            region_fingerprints,
        })
    }

    pub const fn slug(&self) -> &Slug {
        &self.slug
    }

    /// The primary repository.
    pub const fn repository(&self) -> &Repository {
        self.repositories.primary()
    }

    pub const fn repositories(&self) -> &Repositories {
        &self.repositories
    }

    /// This plan with one more declared repository.
    pub fn declaring(
        self,
        alias: RepoAlias,
        repository: Repository,
    ) -> Result<Self, DeclarationError> {
        Ok(Self {
            repositories: self.repositories.with(alias, repository)?,
            ..self
        })
    }

    /// This plan with every repository's identity re-captured.
    pub fn recapturing<E>(
        self,
        capture: impl FnMut(&Repository) -> Result<Repository, E>,
    ) -> Result<Self, E> {
        Ok(Self {
            repositories: self.repositories.recaptured(capture)?,
            ..self
        })
    }

    pub const fn state(&self) -> &RunState {
        &self.state
    }

    pub const fn evidence_by_id(&self) -> &BTreeMap<EvidenceId, EvidenceIndex> {
        &self.evidence_by_id
    }

    pub const fn claim_by_id(&self) -> &BTreeMap<ClaimId, ClaimIndex> {
        &self.claim_by_id
    }

    pub const fn region_fingerprints(&self) -> &BTreeMap<PlanRegion, RegionFingerprint> {
        &self.region_fingerprints
    }

    /// Project the proven state back into a raw document and serialize it.
    pub fn to_canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.raw_document(self.state.to_raw()))
    }

    /// This plan's identity and repositories around `run`.
    pub(crate) fn raw_document(&self, run: RawRunState) -> RawPlanDocument {
        let repositories = &self.repositories;
        let primary = repositories.primary();
        let alias = repositories.primary_alias();
        RawPlanDocument {
            slug: self.slug.clone(),
            repository: primary.clone(),
            repository_alias: (*alias != RepoAlias::for_key(&primary.name)).then(|| alias.clone()),
            repositories: repositories.others().clone(),
            run,
        }
    }
}

/// The full-plan JSON Schema, derived from [`RawPlanDocument`].
pub fn schema() -> serde_json::Value {
    SchemaGenerator::default()
        .into_root_schema_for::<RawPlanDocument>()
        .into()
}

fn deserialize(raw: &[u8]) -> Result<RawPlanDocument, ParsePlanError> {
    let mut deserializer = serde_json::Deserializer::from_slice(raw);
    serde_path_to_error::deserialize::<_, RawPlanDocument>(&mut deserializer)
        .map_err(|error| ParsePlanError::from_serde(&error))
}

/// The unique evidence and claim indexes produced by resolving a document.
type PlanIndexes = (
    BTreeMap<EvidenceId, EvidenceIndex>,
    BTreeMap<ClaimId, ClaimIndex>,
);

fn index_and_resolve(document: &RawPlanDocument) -> Result<PlanIndexes, ParsePlanError> {
    let mut evidence_by_id: BTreeMap<EvidenceId, EvidenceIndex> = BTreeMap::new();
    let mut claim_by_id: BTreeMap<ClaimId, ClaimIndex> = BTreeMap::new();

    let Some((context, base)) = document.run.exploration() else {
        return Ok((evidence_by_id, claim_by_id));
    };

    for (position, evidence) in context.evidence.iter().enumerate() {
        let pointer = format!("{base}/evidence/{position}");
        let id = evidence_id(evidence).clone();
        if !evidence.sample_admits() {
            return Err(ParsePlanError::at(
                pointer,
                format!("evidence `{id}` declares a sample smaller than the items it retained"),
            ));
        }
        if evidence_by_id
            .insert(id.clone(), EvidenceIndex { pointer })
            .is_some()
        {
            return Err(ParsePlanError::at(
                format!("{base}/evidence/{position}/id"),
                format!("duplicate evidence id `{id}`"),
            ));
        }
    }

    for (position, claim) in context.claims.iter().enumerate() {
        let pointer = format!("{base}/claims/{position}");
        if claim_by_id
            .insert(claim.id.clone(), ClaimIndex { pointer })
            .is_some()
        {
            return Err(ParsePlanError::at(
                format!("{base}/claims/{position}/id"),
                format!("duplicate claim id `{}`", claim.id),
            ));
        }
    }

    // Impact subjects are the one region key that is free text rather than a
    // uniqueness-validated id, so a collision would silently drop a region from
    // the fingerprint map.
    let mut impact_subjects: BTreeSet<&NonEmptyText> = BTreeSet::new();
    for (position, assessment) in context.impact.iter().enumerate() {
        if !impact_subjects.insert(&assessment.subject) {
            return Err(ParsePlanError::at(
                format!("{base}/impact/{position}/subject"),
                format!("duplicate impact subject `{}`", assessment.subject),
            ));
        }
    }

    resolve_references(context, base, &evidence_by_id, &claim_by_id)?;

    if let Some((delivery, delivery_base)) = document.run.delivery() {
        resolve_delivery(delivery, &delivery_base, &claim_by_id)?;
    }

    Ok((evidence_by_id, claim_by_id))
}

/// Resolve every exploration cross-reference whose target id-space is
/// materialized in the current snapshot. Uncertainty dispositions
/// (`AcceptedByUser` → decisions, `ContainedByCriterion` → criteria) are proven at
/// [`Candidate::parse`] instead, where a candidate exists to resolve against.
fn resolve_references(
    context: &RawExplorationContext,
    base: &str,
    evidence_by_id: &BTreeMap<EvidenceId, EvidenceIndex>,
    claim_by_id: &BTreeMap<ClaimId, ClaimIndex>,
) -> Result<(), ParsePlanError> {
    for (claim_position, claim) in context.claims.iter().enumerate() {
        for (evidence_position, reference) in claim.evidence.iter().enumerate() {
            if !evidence_by_id.contains_key(reference) {
                return Err(ParsePlanError::at(
                    format!("{base}/claims/{claim_position}/evidence/{evidence_position}"),
                    format!("claim evidence `{reference}` does not resolve to any evidence"),
                ));
            }
        }
    }

    let uncertainty_ids: BTreeSet<_> = context
        .uncertainties
        .iter()
        .map(|uncertainty| &uncertainty.id)
        .collect();

    for (position, assessment) in context.impact.iter().enumerate() {
        match &assessment.entry {
            ImpactEntry::Affected { claims, .. } => {
                for (claim_position, reference) in claims.iter().enumerate() {
                    if !claim_by_id.contains_key(reference) {
                        return Err(ParsePlanError::at(
                            format!("{base}/impact/{position}/claims/{claim_position}"),
                            format!("impact claim `{reference}` does not resolve to any claim"),
                        ));
                    }
                }
            }
            ImpactEntry::Uncertain { uncertainty, .. } => {
                if !uncertainty_ids.contains(uncertainty) {
                    return Err(ParsePlanError::at(
                        format!("{base}/impact/{position}/uncertainty"),
                        format!("impact uncertainty `{uncertainty}` does not resolve"),
                    ));
                }
            }
            ImpactEntry::Unaffected { .. } | ImpactEntry::NotApplicable { .. } => {}
        }
    }

    Ok(())
}

fn resolve_delivery(
    delivery: &DeliveryPlan,
    base: &str,
    claim_by_id: &BTreeMap<ClaimId, ClaimIndex>,
) -> Result<(), ParsePlanError> {
    for (requirement_position, requirement) in delivery.requirements.iter().enumerate() {
        for (claim_position, reference) in requirement.claims.iter().enumerate() {
            if !claim_by_id.contains_key(reference) {
                return Err(ParsePlanError::at(
                    format!("{base}/requirements/{requirement_position}/claims/{claim_position}"),
                    format!("requirement claim `{reference}` does not resolve to any claim"),
                ));
            }
        }
    }
    Ok(())
}

/// The fine-grained region fingerprints of the candidate this document carries, if
/// any. Pre-candidate states have no fingerprintable regions.
fn region_fingerprints(document: &RawPlanDocument) -> BTreeMap<PlanRegion, RegionFingerprint> {
    document
        .run
        .candidate()
        .map(|(candidate, _)| fingerprints(candidate))
        .unwrap_or_default()
}

/// Prove the candidate coherence of every candidate-bearing state (design §6.6),
/// and that a final plan's accepted findings all appear in its review history.
fn prove_candidate(
    document: &RawPlanDocument,
    repositories: &Repositories,
) -> Result<(), ParsePlanError> {
    if let Some((candidate, base)) = document.run.candidate() {
        Candidate::parse(candidate.clone(), repositories)
            .map_err(|issues| ParsePlanError::from_issues(base, &issues))?;
    }
    if let Some((accepted, history)) = document.run.final_accepted_and_history() {
        let known = history_finding_ids(history);
        for (position, finding) in accepted.iter().enumerate() {
            if !known.contains(&finding.id) {
                return Err(ParsePlanError::at(
                    format!("/run/data/accepted/{position}"),
                    format!(
                        "accepted finding `{}` does not appear in the review history",
                        finding.id
                    ),
                ));
            }
        }
    }
    Ok(())
}

const fn evidence_id(evidence: &Evidence) -> &EvidenceId {
    match evidence {
        Evidence::Runtime(evidence) => &evidence.id,
        Evidence::Semantic(evidence) => &evidence.id,
        Evidence::Structural(evidence) => &evidence.id,
        Evidence::Textual(evidence) => &evidence.id,
        Evidence::Documentation(evidence) => &evidence.id,
        Evidence::History(evidence) => &evidence.id,
        Evidence::Inference(evidence) => &evidence.id,
    }
}

fn json_pointer_from_path(path: &serde_path_to_error::Path) -> String {
    let mut pointer = String::new();
    for segment in path {
        match segment {
            Segment::Seq { index } => {
                pointer.push('/');
                pointer.push_str(&index.to_string());
            }
            Segment::Map { key } => {
                pointer.push('/');
                pointer.push_str(&escape_pointer_segment(key));
            }
            Segment::Enum { .. } | Segment::Unknown => {}
        }
    }
    pointer
}

fn append_segment(base: &str, field: &str) -> String {
    format!("{base}/{}", escape_pointer_segment(field))
}

fn escape_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn extract_backtick_field(message: &str, prefix: &str) -> Option<String> {
    let after_prefix = message.get(message.find(prefix)? + prefix.len()..)?;
    let start = after_prefix.find('`')? + 1;
    let rest = after_prefix.get(start..)?;
    let end = rest.find('`')? + start;
    after_prefix.get(start..end).map(str::to_owned)
}
