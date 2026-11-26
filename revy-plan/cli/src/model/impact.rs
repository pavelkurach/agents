use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{ClaimId, UncertaintyId};
use crate::primitives::{NonEmptyText, OneOrMore};

/// How an affected subject changes, so a reviewer can tell a risk-bearing
/// behavioral change from a mechanical rewrap at a glance. Closed: every
/// affected subject has exactly one shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AffectedShape {
    /// Observable semantics change — the highest-scrutiny case.
    Behavioral,
    /// A type/signature rewrap; bytes and behavior are unchanged.
    Mechanical,
    /// New surface only; nothing existing changes.
    Additive,
    /// A change confined to a stated, bounded region.
    Scoped,
    /// An in-memory / non-durable state-shape change, needing no migration.
    InStore,
}

/// The blast-radius verdict for one area of the codebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImpactEntry {
    Affected {
        rationale: NonEmptyText,
        shape: AffectedShape,
        claims: OneOrMore<ClaimId>,
    },
    Unaffected {
        rationale: NonEmptyText,
        search: NonEmptyText,
    },
    Uncertain {
        rationale: NonEmptyText,
        uncertainty: UncertaintyId,
    },
    NotApplicable {
        rationale: NonEmptyText,
    },
}

/// One area under assessment paired with its impact verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImpactAssessment {
    pub subject: NonEmptyText,
    pub entry: ImpactEntry,
}

/// The closed checklist of standard blast-radius dimensions a complete plan
/// accounts for.
///
/// Subjects stay author-named and uncapped; this closed set is the completeness
/// floor a verified plan is proven total over, so a whole dimension (an
/// unbounded resource, a compatibility break) can never silently go
/// unconsidered.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BlastLane {
    Api,
    DataAtRest,
    Concurrency,
    ResourceBounds,
    Observability,
    Compatibility,
    Configuration,
    Tests,
    Security,
}

impl BlastLane {
    /// Every lane, in declaration order — the closed set a verified plan's
    /// coverage map must be total over (each present exactly once).
    pub const ALL: [Self; 9] = [
        Self::Api,
        Self::DataAtRest,
        Self::Concurrency,
        Self::ResourceBounds,
        Self::Observability,
        Self::Compatibility,
        Self::Configuration,
        Self::Tests,
        Self::Security,
    ];

    /// The lane's wire value — what a result writes, so error texts name lanes
    /// by it.
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::DataAtRest => "data_at_rest",
            Self::Concurrency => "concurrency",
            Self::ResourceBounds => "resource_bounds",
            Self::Observability => "observability",
            Self::Compatibility => "compatibility",
            Self::Configuration => "configuration",
            Self::Tests => "tests",
            Self::Security => "security",
        }
    }

    /// The lane's human-facing label — the wire token spelled for a reader, used
    /// in the rendered coverage strip.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Api => "API",
            Self::DataAtRest => "data at rest",
            Self::Concurrency => "concurrency",
            Self::ResourceBounds => "resource bounds",
            Self::Observability => "observability",
            Self::Compatibility => "compatibility",
            Self::Configuration => "configuration",
            Self::Tests => "tests",
            Self::Security => "security",
        }
    }
}

/// How one blast-radius lane is handled: covered by one or more named impact
/// subjects, or explicitly argued as not considered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum LaneCoverage {
    Covered { subjects: OneOrMore<NonEmptyText> },
    NotConsidered { reason: NonEmptyText },
}

/// One blast-radius lane paired with how the plan handles it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImpactCoverage {
    pub lane: BlastLane,
    pub coverage: LaneCoverage,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn test_affected_entry_requires_a_shape() {
        // `shape` is required, not defaulted: an affected verdict with no shape
        // is an incomplete assessment, not a blank one.
        let json = r#"{"state":"affected","rationale":"Callers change.","claims":["claim-1"]}"#;

        serde_json::from_str::<ImpactEntry>(json).expect_err("a missing shape must be rejected");
    }

    #[test]
    fn test_affected_entry_round_trips_with_its_shape() {
        let json = r#"{"state":"affected","rationale":"Callers change.","shape":"mechanical","claims":["claim-1"]}"#;

        let entry: ImpactEntry = serde_json::from_str(json).expect("an affected entry parses");
        match &entry {
            ImpactEntry::Affected { shape, .. } => assert_eq!(*shape, AffectedShape::Mechanical),
            other => panic!("expected an affected entry, got {other:?}"),
        }
        let reserialized = serde_json::to_string(&entry).expect("serializes");
        assert_eq!(
            serde_json::from_str::<ImpactEntry>(&reserialized).expect("re-parses"),
            entry
        );
    }

    #[test]
    fn test_unaffected_entry_takes_no_shape() {
        // Only `Affected` carries a shape; adding one to another verdict is an
        // unknown field, rejected by `deny_unknown_fields`.
        let json = r#"{"state":"unaffected","rationale":"No caller changes.","search":"grep","shape":"mechanical"}"#;

        serde_json::from_str::<ImpactEntry>(json)
            .expect_err("a shape on a non-affected verdict must be rejected");
    }

    #[test]
    fn test_blast_lane_all_lists_every_variant_once() {
        // `ALL` is the closed set the coverage gate is total over; a variant
        // missing here would silently drop from the completeness floor.
        assert_eq!(BlastLane::ALL.len(), 9);
        let unique: std::collections::BTreeSet<_> = BlastLane::ALL.iter().collect();
        assert_eq!(unique.len(), 9, "every lane in ALL must be distinct");
    }

    #[test]
    fn test_covered_lane_rejects_an_empty_subjects_list() {
        // A covered lane must name at least one subject (`OneOrMore`); an empty
        // list is a lane claimed covered by nothing.
        let json = r#"{"lane":"api","coverage":{"state":"covered","subjects":[]}}"#;

        serde_json::from_str::<ImpactCoverage>(json)
            .expect_err("a covered lane with no subjects must be rejected");
    }

    #[test]
    fn test_impact_coverage_round_trips() {
        for json in [
            r#"{"lane":"resource_bounds","coverage":{"state":"covered","subjects":["buffering"]}}"#,
            r#"{"lane":"security","coverage":{"state":"not_considered","reason":"No new trust boundary."}}"#,
        ] {
            let entry: ImpactCoverage = serde_json::from_str(json).expect("coverage parses");
            let reserialized = serde_json::to_string(&entry).expect("serializes");
            assert_eq!(
                serde_json::from_str::<ImpactCoverage>(&reserialized).expect("re-parses"),
                entry
            );
        }
    }
}
