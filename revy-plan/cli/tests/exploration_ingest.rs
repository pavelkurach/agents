//! Exploration ingest fails early and reports everything: grounding on every
//! exploration apply, coverage once exploration completes, every defect listed
//! with its pointer, approved-tool provenance by id or name, recorded tool
//! failures, and repository-identity stamping of evidence that omits `target`.
#![allow(
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use common::{
    DIGEST, REVISION, StepError, apply_ok, check_step, check_step_against, document, parse, report,
    simple_candidate, step, step_against,
};
use revy_plan::document::ParsedPlan;
use revy_plan::packet::WorkKind;
use revy_plan::runtime::{CheckError, Checkout};
use revy_plan::state::{ExplorationContextError, ExplorationProgress, RunState};
use revy_plan::workflow::{ApplyReason, frontier};
use serde_json::{Value, json};

fn exploring(approved: Value) -> ParsedPlan {
    parse(&document(
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": approved },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        }),
    ))
}

fn doc_evidence(id: &str, target: Value) -> Value {
    json!({
        "kind": "documentation", "id": id, "document": "README.md", "section": "Queue",
        "observation": "The queue is bounded.", "target": target, "limitation": null
    })
}

fn grounded() -> Value {
    json!({ "revision": REVISION, "dirty_digest": DIGEST })
}

fn serena_evidence(tool: &str) -> Value {
    json!({
        "kind": "structural", "id": "evidence-imports", "provenance": { "tool": tool, "version": null },
        "subject": "queue", "observation": { "shape": "imports", "modules": ["std::sync"] },
        "target": grounded(), "limitation": null
    })
}

/// The complete exploration of the fixture candidate, finishing exploration.
fn complete_exploration() -> Value {
    let mut result = simple_candidate()["exploration"].clone();
    result["next"] = json!({ "kind": "complete" });
    result
}

fn collecting_exploration(evidence: Value) -> Value {
    json!({
        "evidence": evidence, "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "propose_capability", "report": report() }
    })
}

fn rejection(error: &StepError) -> Vec<(String, ApplyReason)> {
    let StepError::Apply(error) = error else {
        panic!("expected an apply rejection, got {error:?}");
    };
    error
        .reasons()
        .iter()
        .map(|located| {
            (
                located.pointer().as_str().to_owned(),
                located.error().clone(),
            )
        })
        .collect()
}

// --- A1: fail early --------------------------------------------------------------

#[test]
fn completing_exploration_with_a_missing_lane_is_rejected_at_the_exploration_apply() {
    let plan = exploring(json!([]));
    let mut result = complete_exploration();
    result["coverage"].as_array_mut().unwrap().pop();

    let error = step(plan, WorkKind::Exploration, &result).expect_err("incomplete coverage");

    assert_eq!(
        rejection(&error),
        vec![(
            "/coverage".to_owned(),
            ApplyReason::InvalidExploration(ExplorationContextError::MissingLanes {
                lanes: vec![revy_plan::model::impact::BlastLane::Security],
            }),
        )]
    );
}

#[test]
fn a_missing_lane_is_named_by_its_wire_value() {
    let plan = exploring(json!([]));
    let mut result = complete_exploration();
    result["coverage"].as_array_mut().unwrap().remove(1);

    let error = step(plan, WorkKind::Exploration, &result).expect_err("incomplete coverage");

    assert_eq!(
        rejection(&error)[0].1.to_string(),
        "exploration result is not internally consistent: blast lane(s) `data_at_rest` missing from the coverage checklist"
    );
}

#[test]
fn a_duplicate_lane_is_named_by_its_wire_value() {
    let plan = exploring(json!([]));
    let mut result = complete_exploration();
    let first = result["coverage"][0].clone();
    result["coverage"].as_array_mut().unwrap().push(first);

    let error = step(plan, WorkKind::Exploration, &result).expect_err("duplicate lane");

    assert_eq!(
        rejection(&error)[0].1.to_string(),
        "exploration result is not internally consistent: duplicate blast lane `api`"
    );
}

#[test]
fn an_exploration_that_is_still_collecting_owes_no_coverage() {
    let plan = exploring(json!([]));
    let result = collecting_exploration(json!([doc_evidence("evidence-doc", grounded())]));

    let next = apply_ok(plan, WorkKind::Exploration, &result);

    assert!(matches!(
        next.state(),
        RunState::AwaitingCapabilityApproval { .. }
    ));
}

#[test]
fn stale_evidence_is_rejected_at_a_collecting_exploration_apply() {
    let plan = exploring(json!([]));
    let stale = json!({ "revision": "rev0", "dirty_digest": DIGEST });
    let result = collecting_exploration(json!([doc_evidence("evidence-doc", stale)]));

    let error = step(plan, WorkKind::Exploration, &result).expect_err("stale evidence");

    assert_eq!(
        error.reason().to_string(),
        format!(
            "exploration result is not internally consistent: STALE-MODEL: evidence `evidence-doc` was observed against revision `rev0` (dirty digest `{DIGEST}`), but the plan's repository `fixture-repo` is at revision `rev1` (dirty digest `{DIGEST}`)"
        )
    );
}

#[test]
fn a_complete_coherent_exploration_advances_to_the_design_brief() {
    let plan = exploring(json!([]));

    let next = apply_ok(plan, WorkKind::Exploration, &complete_exploration());

    assert!(matches!(
        next.state(),
        RunState::Exploring {
            progress: ExplorationProgress::Complete { .. },
            ..
        }
    ));
}

// --- A2: every defect, located ---------------------------------------------------

#[test]
fn an_exploration_with_independent_defects_reports_each_at_its_pointer() {
    let plan = exploring(json!([]));
    let mut result = collecting_exploration(json!([
        doc_evidence("evidence-doc", grounded()),
        doc_evidence("evidence-doc", grounded()),
    ]));
    result["claims"] = json!([{
        "id": "claim-1", "statement": "Bounded.", "evidence": ["evidence-missing"],
        "consequences": ["Blocks."], "conclusion": { "state": "established" }
    }]);

    let error = step(plan, WorkKind::Exploration, &result).expect_err("two defects");

    let pointers: Vec<String> = rejection(&error).into_iter().map(|(p, _)| p).collect();
    assert_eq!(pointers, vec!["/evidence/1/id", "/claims/0/evidence/0"]);
}

#[test]
fn a_draft_with_independent_candidate_defects_reports_each_at_its_pointer() {
    let candidate = simple_candidate();
    let plan = parse(&common::drafting_document(&candidate));
    let mut draft = common::draft_result(&candidate);
    draft["content"]["changed_symbols"] = json!([
        { "symbol": "push", "path": "src/unlisted.rs", "change": "added" },
        { "symbol": "push", "path": "src/queue.rs", "change": "added" }
    ]);

    let error = step(plan, WorkKind::Draft, &draft).expect_err("two defects");

    let pointers: Vec<String> = rejection(&error).into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        pointers,
        vec![
            "/content/changed_symbols/0/path",
            "/content/changed_symbols/1/symbol"
        ]
    );
}

#[test]
fn the_runtime_check_reports_every_missing_anchor() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let mut document_value = document(
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": [] },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        }),
    );
    document_value["repository"]["root"] = json!(repo.path().to_str().unwrap());
    let plan = parse(&document_value);
    let result = json!({
        "evidence": [{
            "kind": "textual", "id": "evidence-text", "scope": ["src/a.rs", "src/b.rs"],
            "pattern": "queue", "matches": [], "completeness": { "kind": "complete" },
            "target": grounded(), "limitation": null
        }],
        "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "propose_capability", "report": report() }
    });

    let issues = check_step(&plan, WorkKind::Exploration, &result, repo.path())
        .expect_err("two absent scopes");

    assert_eq!(issues.len(), 2);
}

#[test]
fn a_missing_anchor_names_the_target_root_it_resolved_against() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let mut document_value = document(
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": [] },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        }),
    );
    let root = repo.path().to_str().unwrap().to_owned();
    document_value["repository"]["root"] = json!(root);
    let plan = parse(&document_value);
    let result = json!({
        "evidence": [{
            "kind": "textual", "id": "evidence-text", "scope": ["src/a.rs"],
            "pattern": "queue", "matches": [], "completeness": { "kind": "complete" },
            "target": grounded(), "limitation": null
        }],
        "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "propose_capability", "report": report() }
    });

    let issues =
        check_step(&plan, WorkKind::Exploration, &result, repo.path()).expect_err("absent scope");

    assert_eq!(
        issues.first().error().to_string(),
        format!(
            "evidence `evidence-text` names source anchor `src/a.rs` that is not present under target root `{root}`: no such path"
        )
    );
}

// --- A3: approved-tool provenance ---------------------------------------------------

#[test]
fn an_approved_tool_cited_by_its_proposal_id_satisfies_the_usage_check() {
    let plan = exploring(json!(["proposal-serena"]));
    let result = collecting_exploration(json!([serena_evidence("proposal-serena")]));

    let outcome = check_step(&plan, WorkKind::Exploration, &result, &std::env::temp_dir());

    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn an_approved_tool_cited_by_its_name_satisfies_the_usage_check() {
    let plan = exploring(json!(["proposal-serena"]));
    let result = collecting_exploration(json!([serena_evidence("serena")]));

    let outcome = check_step(&plan, WorkKind::Exploration, &result, &std::env::temp_dir());

    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn a_runtime_command_running_the_approved_program_satisfies_the_usage_check() {
    let transcripts = tempfile::tempdir().expect("transcripts");
    std::fs::write(transcripts.path().join("run.txt"), "ok\n").expect("artifact");
    let digest = revy_plan::primitives::Sha256Digest::hash(b"ok\n");
    let plan = exploring(json!(["proposal-serena"]));
    let result = collecting_exploration(json!([{
        "kind": "runtime", "id": "evidence-run",
        "command": { "kind": "program", "program": "serena", "args": ["index"] },
        "target": grounded(), "observation": "Indexed.",
        "artifact": { "path": "run.txt", "digest": digest.as_str() }, "limitation": null
    }]));

    let outcome = check_step(&plan, WorkKind::Exploration, &result, transcripts.path());

    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn an_unused_approved_tool_names_the_provenance_seen_and_the_accepted_names() {
    let plan = exploring(json!(["proposal-serena"]));
    let result = collecting_exploration(json!([serena_evidence("rg")]));

    let issues = check_step(&plan, WorkKind::Exploration, &result, &std::env::temp_dir())
        .expect_err("serena unused");

    assert_eq!(
        issues.first().error().to_string(),
        "approved tool `proposal-serena` (\"serena\") is unused; provenance seen: [\"rg\"]; set provenance.tool (or a runtime command's program) to one of [\"proposal-serena\", \"serena\"] on the evidence it produced, or record a `tool_failures` entry {\"proposal\": \"proposal-serena\", \"reason\": ...}"
    );
}

#[test]
fn tool_provenance_matching_is_case_sensitive() {
    let plan = exploring(json!(["proposal-serena"]));
    let result = collecting_exploration(json!([serena_evidence("Serena")]));

    let issues = check_step(&plan, WorkKind::Exploration, &result, &std::env::temp_dir())
        .expect_err("case differs");

    assert!(matches!(
        issues.first().error(),
        CheckError::ApprovedToolUnused { .. }
    ));
}

// --- A4: approved-but-failed tools ---------------------------------------------------

fn failed_serena_exploration(proposal: &str) -> Value {
    let mut result = collecting_exploration(json!([doc_evidence("evidence-doc", grounded())]));
    result["tool_failures"] =
        json!([{ "proposal": proposal, "reason": "The server crashed on start." }]);
    result
}

#[test]
fn a_recorded_tool_failure_satisfies_the_usage_check() {
    let plan = exploring(json!(["proposal-serena"]));

    let outcome = check_step(
        &plan,
        WorkKind::Exploration,
        &failed_serena_exploration("proposal-serena"),
        &std::env::temp_dir(),
    );

    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn a_recorded_tool_failure_is_kept_on_the_exploration_context() {
    let plan = exploring(json!(["proposal-serena"]));

    let next = apply_ok(
        plan,
        WorkKind::Exploration,
        &failed_serena_exploration("proposal-serena"),
    );

    let RunState::AwaitingCapabilityApproval {
        prior: Some(prior), ..
    } = next.state()
    else {
        panic!("expected an amendment carrying the prior context");
    };
    assert_eq!(
        prior.tool_failures()[0].proposal.as_str(),
        "proposal-serena"
    );
}

#[test]
fn a_tool_failure_naming_an_unapproved_proposal_is_rejected() {
    let plan = exploring(json!([]));

    let error = step(
        plan,
        WorkKind::Exploration,
        &failed_serena_exploration("proposal-ghost"),
    )
    .expect_err("unknown proposal");

    assert_eq!(
        rejection(&error)[0].0,
        "/tool_failures/0/proposal".to_owned()
    );
}

// --- A6: repository-identity stamping -------------------------------------------------

#[test]
fn evidence_without_a_target_is_stamped_with_the_plan_identity_when_the_checkout_is_unmoved() {
    let plan = exploring(json!([]));
    let result = collecting_exploration(json!([doc_evidence("evidence-doc", Value::Null)]));

    let next = apply_ok(plan, WorkKind::Exploration, &result);

    let RunState::AwaitingCapabilityApproval {
        prior: Some(prior), ..
    } = next.state()
    else {
        panic!("expected an amendment carrying the prior context");
    };
    assert_eq!(
        serde_json::to_value(prior.evidence()[0].observed_target()).unwrap(),
        grounded()
    );
}

#[test]
fn evidence_without_a_target_is_rejected_naming_both_revisions_when_the_checkout_moved() {
    let plan = exploring(json!([]));
    let result = collecting_exploration(json!([doc_evidence("evidence-doc", Value::Null)]));
    let moved = Checkout::Captured(
        serde_json::from_value(json!({ "revision": "rev2", "dirty_digest": DIGEST })).unwrap(),
    );

    let issues = check_step_against(
        &plan,
        WorkKind::Exploration,
        &result,
        &std::env::temp_dir(),
        &moved,
    )
    .expect_err("moved checkout");

    assert_eq!(
        issues.first().error().to_string(),
        format!(
            "evidence `evidence-doc` omits `target`, but the checkout of repository `fixture-repo` (`/tmp/fixture-repo`) moved since the plan was captured: plan revision `rev1` (dirty digest `{DIGEST}`), checkout now at revision `rev2` (dirty digest `{DIGEST}`); restore the checkout, or run `revy-plan reopen exploration --recapture --reason TEXT` to re-capture it and re-verify the carried evidence against it"
        )
    );
}

#[test]
fn evidence_with_an_explicit_target_needs_no_live_checkout() {
    let plan = exploring(json!([]));
    let result = collecting_exploration(json!([doc_evidence("evidence-doc", grounded())]));
    let unavailable = Checkout::Unavailable {
        detail: "no such directory".to_owned(),
    };

    let outcome = step_against(
        plan,
        WorkKind::Exploration,
        &result,
        &std::env::temp_dir(),
        &unavailable,
    );

    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn the_exploration_packet_context_carries_the_target_identity() {
    let plan = exploring(json!([]));

    let frontier = frontier(&plan);

    assert_eq!(
        frontier.only().unwrap().context()["target_identity"],
        grounded()
    );
}

#[test]
fn a_correction_whose_evidence_omits_its_target_is_stamped_too() {
    let plan = parse(&common::reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let correcting = apply_ok(
        plan,
        WorkKind::IntegratedReview,
        &common::blocking_integrated_review("tests", "finding-1"),
    );
    let mut corrected = simple_candidate();
    corrected["brief"]["summary"] = json!("Add a bounded queue with backpressure");
    corrected["exploration"]["evidence"][0]["target"] = Value::Null;
    let result = json!({
        "candidate": corrected,
        "triage": [{ "finding": "finding-1", "outcome": { "kind": "fix", "affected": [{ "region": "brief" }] } }]
    });

    let next = apply_ok(correcting, WorkKind::Correction, &result);

    let candidate = next.state().candidate().expect("a candidate");
    assert_eq!(
        serde_json::to_value(candidate.raw().exploration.evidence[0].observed_target()).unwrap(),
        grounded()
    );
}

#[test]
fn a_correction_reports_candidate_and_triage_defects_together() {
    let plan = parse(&common::reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let correcting = apply_ok(
        plan,
        WorkKind::IntegratedReview,
        &common::blocking_integrated_review("tests", "finding-1"),
    );
    let mut corrected = simple_candidate();
    corrected["content"]["changed_symbols"][0]["path"] = json!("src/unlisted.rs");
    let result = json!({ "candidate": corrected, "triage": [] });

    let error = step(correcting, WorkKind::Correction, &result).expect_err("two defects");

    let pointers: Vec<String> = rejection(&error).into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        pointers,
        vec!["/candidate/content/changed_symbols/0/path", "/triage"]
    );
}
