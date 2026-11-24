//! Exploration completeness (design §6.1) and the brief → user-decision →
//! drafting chain that persists decisions and the user's answers (design §6.4).
#![allow(
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use common::{
    DIGEST, apply_ok, document, parse, report, reviewing_document, simple_candidate, step,
    try_parse,
};
use revy_plan::model::content::{DecisionOwner, DecisionState};
use revy_plan::packet::WorkKind;
use revy_plan::state::RunState;
use revy_plan::workflow::ApplyReason;
use serde_json::{Value, json};

fn sampled_semantic(total: u32) -> Value {
    json!({
        "kind": "semantic", "id": "evidence-refs", "provenance": { "tool": "grep", "version": null },
        "operation": "find_referencing_symbols", "subject": "push",
        "observation": {
            "shape": "references",
            "references": [{ "location": { "path": "src/queue.rs", "line": 1, "column": 1 }, "context": null }],
            "completeness": { "kind": "sampled", "total": total }
        },
        "target": { "revision": "rev1", "dirty_digest": DIGEST }, "limitation": null
    })
}

fn candidate_with_extra_evidence(evidence: Value) -> Value {
    let mut candidate = simple_candidate();
    candidate["exploration"]["evidence"]
        .as_array_mut()
        .unwrap()
        .push(evidence);
    candidate
}

// --- Completeness --------------------------------------------------------------

#[test]
fn a_sample_smaller_than_it_retained_is_rejected_at_load() {
    let candidate = candidate_with_extra_evidence(sampled_semantic(0));
    let error = try_parse(&reviewing_document(
        candidate,
        json!([{ "kind": "integrated" }]),
    ))
    .expect_err("a dishonest sample is rejected");
    assert!(error.to_string().contains("sample smaller"), "{error}");
}

#[test]
fn a_references_observation_without_completeness_is_rejected_at_load() {
    let mut evidence = sampled_semantic(5);
    evidence["observation"]
        .as_object_mut()
        .unwrap()
        .remove("completeness");
    let candidate = candidate_with_extra_evidence(evidence);
    let error = try_parse(&reviewing_document(
        candidate,
        json!([{ "kind": "integrated" }]),
    ))
    .expect_err("a missing completeness is rejected");
    assert!(error.to_string().contains("completeness"), "{error}");
}

#[test]
fn an_honest_sample_is_accepted() {
    let candidate = candidate_with_extra_evidence(sampled_semantic(5));
    parse(&reviewing_document(
        candidate,
        json!([{ "kind": "integrated" }]),
    ));
}

// --- Brief and decisions -------------------------------------------------------

fn exploring_complete() -> Value {
    document(
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": [] },
            "progress": { "phase": "complete", "context": simple_candidate()["exploration"] }
        }),
    )
}

fn brief_result() -> Value {
    json!({
        "brief": {
            "title": "Bounded queue", "summary": "Add a bounded queue",
            "scope": "queue", "out_of_scope": "nothing"
        },
        "decisions": [
            { "owner": "planner", "id": "decision-container", "question": "Which container?",
              "answer": "VecDeque", "consequences": ["No new dependency."], "ruled_out": [] },
            { "owner": "user", "id": "decision-bound", "question": "Which bound?",
              "alternatives": ["1024", "4096"], "consequences": ["Producers block when full."] }
        ]
    })
}

#[test]
fn a_brief_with_a_user_decision_awaits_the_user() {
    let plan = parse(&exploring_complete());
    let next = apply_ok(plan, WorkKind::DesignBrief, &brief_result());
    match next.state() {
        RunState::AwaitingUserDecision {
            decisions,
            questions,
            ..
        } => {
            assert_eq!(decisions.as_slice().len(), 2);
            assert_eq!(questions.as_slice().len(), 1);
        }
        other => panic!("expected awaiting user decision, got {other:?}"),
    }
}

#[test]
fn answering_the_user_decision_persists_every_decision_answered() {
    let plan = parse(&exploring_complete());
    let awaiting = apply_ok(plan, WorkKind::DesignBrief, &brief_result());
    let answer = json!({
        "decision": "answer",
        "answers": [{ "decision": "decision-bound", "chosen": "1024", "note": null }]
    });
    let drafting = apply_ok(awaiting, WorkKind::UserDecision, &answer);
    let RunState::Drafting { decisions, .. } = drafting.state() else {
        panic!("expected drafting")
    };
    assert_eq!(decisions.len(), 2);
    let user = decisions
        .iter()
        .find(|d| matches!(d.owner, DecisionOwner::User))
        .unwrap();
    match &user.state {
        DecisionState::Answered {
            answer, ruled_out, ..
        } => {
            assert_eq!(answer.as_str(), "1024");
            // The alternative the user did not choose is ruled out.
            assert!(
                ruled_out
                    .iter()
                    .any(|entry| entry.alternative.as_str() == "4096")
            );
        }
        DecisionState::Proposed => panic!("the user decision must be answered"),
    }
}

#[test]
fn answers_that_do_not_cover_the_questions_are_rejected() {
    let plan = parse(&exploring_complete());
    let awaiting = apply_ok(plan, WorkKind::DesignBrief, &brief_result());
    let answer = json!({
        "decision": "answer",
        "answers": [{ "decision": "decision-unknown", "chosen": "1024", "note": null }]
    });
    let error = step(awaiting, WorkKind::UserDecision, &answer)
        .expect_err("mismatched answers are rejected");
    assert!(matches!(error.reason(), ApplyReason::AnswerMismatch { .. }));
}

#[test]
fn choosing_an_option_the_question_never_offered_is_rejected() {
    let plan = parse(&exploring_complete());
    let awaiting = apply_ok(plan, WorkKind::DesignBrief, &brief_result());
    let answer = json!({
        "decision": "answer",
        "answers": [{ "decision": "decision-bound", "chosen": "9999", "note": null }]
    });
    let error = step(awaiting, WorkKind::UserDecision, &answer)
        .expect_err("an unlisted option is rejected");
    assert!(matches!(error.reason(), ApplyReason::AnswerMismatch { .. }));
}

#[test]
fn a_planner_only_brief_drafts_directly() {
    let plan = parse(&exploring_complete());
    let mut result = brief_result();
    result["decisions"] = json!([
        { "owner": "planner", "id": "decision-container", "question": "Which container?",
          "answer": "VecDeque", "consequences": ["No new dependency."], "ruled_out": [] }
    ]);
    let next = apply_ok(plan, WorkKind::DesignBrief, &result);
    assert!(matches!(next.state(), RunState::Drafting { .. }));
}
