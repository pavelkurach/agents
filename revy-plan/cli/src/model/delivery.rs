use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{AcId, ClaimId, MrId, RequirementId};
use crate::primitives::{Argument, NonEmptyText, OneOrMore, ProgramName, RepoRelPath};

/// A requirement the plan must satisfy, grounded in supporting claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: RequirementId,
    pub statement: NonEmptyText,
    pub claims: Vec<ClaimId>,
}

/// A command that checks an acceptance criterion or an MR slice.
///
/// This is a *planned* command — nothing runs at planning time (design §6.3).
/// A check the planner actually ran is `Evidence::Runtime`, not this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifyCommand {
    pub program: ProgramName,
    pub args: Vec<Argument>,
    pub cwd: RepoRelPath,
    pub expected_status: i32,
}

/// How an acceptance criterion is verified: by a planned command, or by a manual
/// procedure the reader follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Verify {
    Command(VerifyCommand),
    Manual { procedure: NonEmptyText },
}

/// An acceptance criterion and how it is verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceCriterion {
    pub id: AcId,
    pub statement: NonEmptyText,
    pub verify: Verify,
}

/// One merge request in the delivery graph: an independently landable step.
///
/// It names the files it touches, the delivery items it depends on, and the
/// acceptance criteria it takes sole ownership of, asserts the behavior it stands
/// up on its own, and may carry a command that fails when the slice is absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Mr {
    pub id: MrId,
    pub title: NonEmptyText,
    pub files: OneOrMore<RepoRelPath>,
    pub needs: Vec<MrId>,
    pub satisfies: Vec<AcId>,
    pub standalone_behavior: NonEmptyText,
    #[serde(default)]
    pub verify: Option<VerifyCommand>,
}

/// What the plan commits to deliver: requirements, at least one acceptance
/// criterion, and the merge-request graph that lands them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeliveryPlan {
    pub requirements: Vec<Requirement>,
    pub acceptance_criteria: OneOrMore<AcceptanceCriterion>,
    // A proportional change may land as a single merge request; a plan whose
    // graph has not been authored yet carries none, so an absent field means
    // "no delivery items" rather than an incomplete document.
    #[serde(default)]
    pub mrs: Vec<Mr>,
}

/// A delivery graph that cannot be proven landable.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeliveryError {
    #[error("delivery has no merge requests")]
    Empty,
    #[error("duplicate merge request id `{id}`")]
    DuplicateMr { id: MrId },
    #[error("merge request `{mr}` needs `{need}`, which is not a merge request")]
    UnresolvedNeed { mr: MrId, need: MrId },
    #[error("merge request `{mr}` satisfies unknown acceptance criterion `{criterion}`")]
    UnknownCriterion { mr: MrId, criterion: AcId },
    #[error("acceptance criterion `{criterion}` is not owned by exactly one merge request")]
    Ownership { criterion: AcId },
    #[error("delivery graph contains a dependency cycle")]
    Cycle { remaining: Vec<MrId> },
}

/// A delivery graph proven landable.
///
/// Its invariants: unique ids, resolved needs, exact one-to-one
/// acceptance-criterion ownership, non-empty standalone behavior (guaranteed
/// structurally by [`NonEmptyText`]), and acyclic dependencies. Constructed only
/// by [`DeliveryGraph::parse`], so it can always be scheduled into [`Waves`]
/// without a cycle branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryGraph {
    mrs: OneOrMore<Mr>,
}

/// A dependency-respecting schedule of a proven [`DeliveryGraph`]: each inner
/// group may land in parallel, and every group's needs land in an earlier one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waves(Vec<Vec<MrId>>);

impl Waves {
    pub fn as_slice(&self) -> &[Vec<MrId>] {
        &self.0
    }
}

impl DeliveryGraph {
    /// Prove a delivery plan's merge-request graph is landable, or report the
    /// first defect. Every acceptance criterion the plan lists must be owned by
    /// exactly one merge request, and every dependency must resolve and be
    /// acyclic.
    pub fn parse(plan: &DeliveryPlan) -> Result<Self, DeliveryError> {
        let mrs = OneOrMore::parse(plan.mrs.clone()).map_err(|_| DeliveryError::Empty)?;

        let mut ids: BTreeSet<&MrId> = BTreeSet::new();
        for mr in &mrs {
            if !ids.insert(&mr.id) {
                return Err(DeliveryError::DuplicateMr { id: mr.id.clone() });
            }
        }
        for mr in &mrs {
            for need in &mr.needs {
                if !ids.contains(need) {
                    return Err(DeliveryError::UnresolvedNeed {
                        mr: mr.id.clone(),
                        need: need.clone(),
                    });
                }
            }
        }

        let mut ownership: BTreeMap<&AcId, usize> = plan
            .acceptance_criteria
            .iter()
            .map(|criterion| (&criterion.id, 0))
            .collect();
        for mr in &mrs {
            for criterion in &mr.satisfies {
                match ownership.get_mut(criterion) {
                    Some(count) => *count += 1,
                    None => {
                        return Err(DeliveryError::UnknownCriterion {
                            mr: mr.id.clone(),
                            criterion: criterion.clone(),
                        });
                    }
                }
            }
        }
        for criterion in &plan.acceptance_criteria {
            if ownership.get(&criterion.id) != Some(&1) {
                return Err(DeliveryError::Ownership {
                    criterion: criterion.id.clone(),
                });
            }
        }

        let graph = Self { mrs };
        let (_, remaining) = graph.schedule();
        if remaining.is_empty() {
            Ok(graph)
        } else {
            Err(DeliveryError::Cycle { remaining })
        }
    }

    /// The merge requests in this graph.
    pub const fn mrs(&self) -> &OneOrMore<Mr> {
        &self.mrs
    }

    /// Schedule the graph into dependency-respecting waves. The graph is proven
    /// acyclic, so scheduling is total; any leftover items are impossible and
    /// simply absent from the result.
    pub fn waves(&self) -> Waves {
        Waves(self.schedule().0)
    }

    /// Kahn's algorithm over the resolved `needs` edges. Returns the scheduled
    /// waves and any items that never became ready (empty for an acyclic graph).
    fn schedule(&self) -> (Vec<Vec<MrId>>, Vec<MrId>) {
        let by_id: BTreeMap<&MrId, &Mr> = self.mrs.iter().map(|mr| (&mr.id, mr)).collect();
        let mut unresolved: BTreeMap<&MrId, usize> = self
            .mrs
            .iter()
            .map(|mr| {
                (
                    &mr.id,
                    mr.needs.iter().filter(|n| by_id.contains_key(n)).count(),
                )
            })
            .collect();
        let mut dependents: BTreeMap<&MrId, Vec<&MrId>> = BTreeMap::new();
        for mr in &self.mrs {
            for need in &mr.needs {
                if by_id.contains_key(need) {
                    dependents.entry(need).or_default().push(&mr.id);
                }
            }
        }

        let mut ready: Vec<&MrId> = self
            .mrs
            .iter()
            .filter(|mr| unresolved.get(&mr.id) == Some(&0))
            .map(|mr| &mr.id)
            .collect();
        let mut waves: Vec<Vec<MrId>> = Vec::new();
        while !ready.is_empty() {
            let wave = std::mem::take(&mut ready);
            for id in &wave {
                if let Some(next) = dependents.get(id) {
                    for dependent in next {
                        if let Some(count) = unresolved.get_mut(*dependent) {
                            *count = count.saturating_sub(1);
                            if *count == 0 {
                                ready.push(dependent);
                            }
                        }
                    }
                }
            }
            waves.push(wave.iter().map(|id| (*id).clone()).collect());
        }

        let remaining: Vec<MrId> = self
            .mrs
            .iter()
            .filter(|mr| unresolved.get(&mr.id).is_some_and(|count| *count > 0))
            .map(|mr| mr.id.clone())
            .collect();
        (waves, remaining)
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]
    use super::*;

    fn text(value: &str) -> NonEmptyText {
        NonEmptyText::parse(value).expect("non-empty")
    }

    fn ac(id: &str) -> AcceptanceCriterion {
        AcceptanceCriterion {
            id: AcId::parse(id).expect("valid id"),
            statement: text("does a thing"),
            verify: Verify::Manual {
                procedure: text("check it by hand"),
            },
        }
    }

    fn mr(id: &str, needs: &[&str], satisfies: &[&str]) -> Mr {
        Mr {
            id: MrId::parse(id).expect("valid id"),
            title: text("a merge request"),
            files: OneOrMore::parse(vec![RepoRelPath::parse("src/lib.rs").expect("valid path")])
                .expect("one file"),
            needs: needs
                .iter()
                .map(|n| MrId::parse(n).expect("valid id"))
                .collect(),
            satisfies: satisfies
                .iter()
                .map(|a| AcId::parse(a).expect("valid id"))
                .collect(),
            standalone_behavior: text("stands on its own"),
            verify: None,
        }
    }

    fn plan(criteria: Vec<AcceptanceCriterion>, mrs: Vec<Mr>) -> DeliveryPlan {
        DeliveryPlan {
            requirements: Vec::new(),
            acceptance_criteria: OneOrMore::parse(criteria).expect("one criterion"),
            mrs,
        }
    }

    #[test]
    fn a_single_merge_request_owning_the_only_criterion_lands_in_one_wave() {
        let graph = DeliveryGraph::parse(&plan(vec![ac("ac-1")], vec![mr("mr-1", &[], &["ac-1"])]))
            .expect("coherent single-item delivery");

        let waves = graph.waves();

        assert_eq!(waves.as_slice().len(), 1);
        assert_eq!(waves.as_slice()[0], vec![MrId::parse("mr-1").unwrap()]);
    }

    #[test]
    fn a_dependency_edge_orders_the_needing_item_into_a_later_wave() {
        let graph = DeliveryGraph::parse(&plan(
            vec![ac("ac-1"), ac("ac-2")],
            vec![
                mr("mr-base", &[], &["ac-1"]),
                mr("mr-dependent", &["mr-base"], &["ac-2"]),
            ],
        ))
        .expect("coherent two-item delivery");

        let waves = graph.waves();

        assert_eq!(waves.as_slice()[0], vec![MrId::parse("mr-base").unwrap()]);
        assert_eq!(
            waves.as_slice()[1],
            vec![MrId::parse("mr-dependent").unwrap()]
        );
    }

    #[test]
    fn a_criterion_no_merge_request_satisfies_is_rejected() {
        let error = DeliveryGraph::parse(&plan(
            vec![ac("ac-owned"), ac("ac-orphan")],
            vec![mr("mr-1", &[], &["ac-owned"])],
        ))
        .expect_err("orphan criterion has no owner");

        assert_eq!(
            error,
            DeliveryError::Ownership {
                criterion: AcId::parse("ac-orphan").unwrap(),
            }
        );
    }

    #[test]
    fn a_criterion_owned_twice_is_rejected() {
        let error = DeliveryGraph::parse(&plan(
            vec![ac("ac-1")],
            vec![mr("mr-a", &[], &["ac-1"]), mr("mr-b", &[], &["ac-1"])],
        ))
        .expect_err("criterion owned by two merge requests");

        assert_eq!(
            error,
            DeliveryError::Ownership {
                criterion: AcId::parse("ac-1").unwrap(),
            }
        );
    }

    #[test]
    fn a_dependency_cycle_is_rejected() {
        let error = DeliveryGraph::parse(&plan(
            vec![ac("ac-1"), ac("ac-2")],
            vec![
                mr("mr-a", &["mr-b"], &["ac-1"]),
                mr("mr-b", &["mr-a"], &["ac-2"]),
            ],
        ))
        .expect_err("cyclic needs");

        assert!(matches!(error, DeliveryError::Cycle { .. }));
    }
}
