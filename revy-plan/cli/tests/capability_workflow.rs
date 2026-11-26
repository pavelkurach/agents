//! The capability-approval phase (design §6.7, §7): approval binds to the report
//! digest, undisclosed proposals are rejected, a revision replaces the report, an
//! amendment retains the prior evidence, and an approved-but-unused tool fails the
//! next exploration.
#![allow(
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use std::path::Path;

use common::{
    DIGEST, REVISION, apply_ok, check_step, document, parse, report, report_digest, repository,
    step,
};
use revy_plan::document::ParsedPlan;
use revy_plan::packet::WorkKind;
use revy_plan::runtime::CheckError;
use revy_plan::state::RunState;
use serde_json::{Value, json};

fn awaiting() -> Value {
    document(
        "awaiting_capability_approval",
        json!({ "report": report(), "prior": null }),
    )
}

fn min_exploration(next: Value) -> Value {
    json!({
        "evidence": [{
            "kind": "documentation", "id": "evidence-doc", "document": "README.md",
            "section": "Queue", "observation": "The queue is bounded.",
            "target": null, "limitation": null
        }],
        "claims": [], "uncertainties": [], "impact": [], "coverage": [], "next": next
    })
}

#[test]
fn proceeding_on_current_tools_enters_exploration() {
    let plan = parse(&awaiting());
    let decision = json!({ "decision": "proceed_current", "report": report_digest() });
    let next = apply_ok(plan, WorkKind::CapabilityApproval, &decision);
    assert!(matches!(next.state(), RunState::Exploring { .. }));
}

#[test]
fn a_decision_carrying_the_wrong_report_digest_is_rejected() {
    let plan = parse(&awaiting());
    let decision = json!({ "decision": "proceed_current", "report": DIGEST });
    let error = step(plan, WorkKind::CapabilityApproval, &decision)
        .expect_err("a stale digest is rejected");
    assert!(matches!(
        error.reason(),
        revy_plan::workflow::ApplyReason::DigestMismatch
    ));
}

#[test]
fn approving_an_undisclosed_proposal_is_rejected() {
    let plan = parse(&awaiting());
    let decision = json!({
        "decision": "approve_additions", "report": report_digest(),
        "proposals": ["proposal-unknown"]
    });
    let error = step(plan, WorkKind::CapabilityApproval, &decision)
        .expect_err("an undisclosed proposal is rejected");
    assert!(matches!(
        error.reason(),
        revy_plan::workflow::ApplyReason::UndisclosedProposal { .. }
    ));
}

#[test]
fn a_revision_request_leaves_the_state_awaiting_a_new_report() {
    let plan = parse(&awaiting());
    let decision = json!({
        "decision": "request_revision", "report": report_digest(), "instruction": "add serena"
    });
    let next = apply_ok(plan, WorkKind::CapabilityApproval, &decision);
    assert!(matches!(
        next.state(),
        RunState::AwaitingCapabilityApproval { .. }
    ));
}

#[test]
fn a_capability_report_replaces_the_current_report() {
    let plan = parse(&awaiting());
    // A fresh report with a second proposal.
    let mut revised = report();
    revised["proposals"].as_array_mut().unwrap().push(json!({
        "id": "proposal-lsp", "name": "rust-analyzer", "closes": ["uncertainty-consumers"],
        "benefit": "Resolve types.", "cost": "Downloads a server.",
        "side_effects": [{"kind": "download"}], "recommendation": "optional"
    }));
    let next = apply_ok(plan, WorkKind::CapabilityReport, &revised);
    match next.state() {
        RunState::AwaitingCapabilityApproval { report, .. } => {
            assert_eq!(report.proposals.len(), 2);
        }
        other => panic!("expected awaiting approval, got {other:?}"),
    }
}

#[test]
fn a_mid_exploration_amendment_retains_the_prior_evidence() {
    // Proceed, gather evidence, then propose a capability: the state returns to
    // awaiting approval carrying the evidence gathered so far.
    let plan = parse(&awaiting());
    let exploring = apply_ok(
        plan,
        WorkKind::CapabilityApproval,
        &json!({ "decision": "proceed_current", "report": report_digest() }),
    );
    let amended = apply_ok(
        exploring,
        WorkKind::Exploration,
        &min_exploration(json!({ "kind": "propose_capability", "report": report() })),
    );
    match amended.state() {
        RunState::AwaitingCapabilityApproval {
            prior: Some(prior), ..
        } => {
            assert_eq!(prior.evidence().len(), 1);
        }
        other => panic!("expected amendment with retained prior, got {other:?}"),
    }
}

#[test]
fn an_approved_tool_that_is_never_used_fails_exploration() {
    let plan = parse(&awaiting());
    let exploring = apply_ok(
        plan,
        WorkKind::CapabilityApproval,
        &json!({
            "decision": "approve_additions", "report": report_digest(),
            "proposals": ["proposal-serena"]
        }),
    );
    // The exploration's evidence carries no `serena` provenance.
    let error = check_step(
        &exploring,
        WorkKind::Exploration,
        &min_exploration(json!({ "kind": "complete" })),
        &std::env::temp_dir(),
    )
    .expect_err("an unused approved tool fails exploration");
    assert!(
        matches!(error.first().error(), CheckError::ApprovedToolUnused { .. }),
        "{error:?}"
    );
}

// --- Runtime anchor and transcript-digest guardrails (design §12, §6.6) --------

/// An `Exploring::Collecting` plan rooted at `root`, so its exploration evidence's
/// source anchors resolve against a directory the test controls.
fn exploring_with_root(root: &Path) -> ParsedPlan {
    let mut repo = repository();
    repo["root"] = json!(root.to_str().expect("a utf-8 root path"));
    parse(&json!({
        "slug": "add-store-metrics",
        "repository": repo,
        "run": { "state": "exploring", "data": {
            "capabilities": { "report": report(), "approved": [] },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        } }
    }))
}

/// An exploration result carrying one textual evidence over `scope` with the given
/// matches — the anchored evidence the runtime check verifies against disk.
fn textual_exploration(scope: &str, matches: Value) -> Value {
    json!({
        "evidence": [{
            "kind": "textual", "id": "evidence-text", "scope": [scope], "pattern": "queue",
            "matches": matches, "completeness": { "kind": "complete" },
            "target": null, "limitation": null
        }],
        "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "complete" }
    })
}

/// An exploration result carrying one runtime evidence whose artifact resolves
/// against the transcripts root, with the given declared digest.
fn runtime_exploration(artifact_path: &str, digest: &str) -> Value {
    json!({
        "evidence": [{
            "kind": "runtime", "id": "evidence-build",
            "command": { "kind": "program", "program": "cargo", "args": ["build"] },
            "target": { "revision": REVISION, "dirty_digest": DIGEST },
            "observation": "The workspace builds.",
            "artifact": { "path": artifact_path, "digest": digest },
            "limitation": null
        }],
        "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "complete" }
    })
}

#[test]
fn an_exploration_naming_an_absent_source_anchor_fails_the_runtime_check() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let plan = exploring_with_root(repo.path());
    // `src/absent.rs` does not exist under the (empty) repo root.
    let result = textual_exploration("src/absent.rs", json!([]));
    let error = check_step(&plan, WorkKind::Exploration, &result, repo.path())
        .expect_err("an absent source anchor is rejected");
    assert!(
        matches!(
            error.first().error(),
            CheckError::MissingSourceAnchor { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn an_absence_claim_scoping_a_directory_passes_the_runtime_check() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    std::fs::create_dir_all(repo.path().join("src")).expect("create src dir");
    let plan = exploring_with_root(repo.path());
    // An absence claim (no matches) may scope a whole directory: `src` exists as a
    // directory, so the anchor is present even though it is not a readable file.
    let result = textual_exploration("src", json!([]));
    check_step(&plan, WorkKind::Exploration, &result, repo.path())
        .expect("a directory scope with no matches is a valid absence anchor");
}

#[test]
fn an_exploration_scoping_an_absent_directory_fails_the_runtime_check() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let plan = exploring_with_root(repo.path());
    // `src` does not exist under the (empty) repo root, so the scope is rejected.
    let result = textual_exploration("src", json!([]));
    let error = check_step(&plan, WorkKind::Exploration, &result, repo.path())
        .expect_err("a nonexistent scope is still rejected");
    assert!(
        matches!(
            error.first().error(),
            CheckError::MissingSourceAnchor { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn an_exploration_naming_a_line_past_the_end_of_the_file_fails_the_runtime_check() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    std::fs::create_dir_all(repo.path().join("src")).expect("create src dir");
    std::fs::write(repo.path().join("src/queue.rs"), "line one\nline two\n")
        .expect("write source file");
    let plan = exploring_with_root(repo.path());
    // The file exists with two lines; the match anchors at line 99.
    let matches = json!([{
        "location": { "path": "src/queue.rs", "line": 99, "column": 1 },
        "line": "push(item)"
    }]);
    let result = textual_exploration("src/queue.rs", matches);
    let error = check_step(&plan, WorkKind::Exploration, &result, repo.path())
        .expect_err("a line past the end of the file is rejected");
    assert!(
        matches!(
            error.first().error(),
            CheckError::AnchorLineOutOfRange { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn a_runtime_artifact_whose_digest_does_not_match_fails_the_runtime_check() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let transcripts = tempfile::tempdir().expect("transcripts tempdir");
    std::fs::write(
        transcripts.path().join("build.txt"),
        "actual build output\n",
    )
    .expect("write artifact");
    let plan = exploring_with_root(repo.path());
    // The declared digest is the all-`a` fixture digest, not the file's hash.
    let result = runtime_exploration("build.txt", DIGEST);
    let error = check_step(&plan, WorkKind::Exploration, &result, transcripts.path())
        .expect_err("a mismatched artifact digest is rejected");
    assert!(
        matches!(
            error.first().error(),
            CheckError::ExplorationTranscriptDigestMismatch { .. }
        ),
        "{error:?}"
    );
}
