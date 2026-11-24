use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::primitives::{PrimitiveError, Slug};

macro_rules! id_type {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
            #[serde(transparent)]
            pub struct $name(Slug);

            impl $name {
                pub fn parse(raw: &str) -> Result<Self, PrimitiveError> {
                    Slug::parse(raw).map(Self)
                }

                pub fn as_str(&self) -> &str {
                    self.0.as_str()
                }
            }

            impl std::fmt::Display for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(self.0.as_str())
                }
            }

            impl<'de> Deserialize<'de> for $name {
                fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                    Slug::deserialize(d).map(Self)
                }
            }
        )+
    };
}

id_type!(
    ClaimId,
    AcId,
    RequirementId,
    RiskId,
    FindingId,
    MrId,
    EvidenceId,
    CapabilityId,
    ProposalId,
    UncertaintyId,
    DecisionId,
);

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]
    use super::*;

    #[test]
    fn test_claim_id_rejects_an_empty_identifier() {
        ClaimId::parse("").expect_err("empty identifier is invalid");
    }

    #[test]
    fn test_claim_id_rejects_whitespace() {
        ClaimId::parse("calibration definition").expect_err("whitespace is invalid");
    }

    #[test]
    fn test_claim_id_accepts_a_kebab_identifier() {
        assert_eq!(
            ClaimId::parse("calibration-definition")
                .expect("valid id")
                .as_str(),
            "calibration-definition"
        );
    }

    #[test]
    fn test_ids_of_different_entities_are_distinct_types() {
        let finding = FindingId::parse("q-1").expect("valid id");
        let claim = ClaimId::parse("q-1").expect("valid id");
        assert_eq!(finding.as_str(), claim.as_str());
    }

    #[test]
    fn test_malformed_id_in_json_is_an_error_not_a_silent_skip() {
        serde_json::from_str::<FindingId>(r#""Finding 1""#)
            .expect_err("malformed finding id must fail loudly");
    }
}
