//! Shared fixture builders for the revy-plan test suite.
//!
//! Documents are built as `serde_json::Value` and parsed through the real
//! `ParsedPlan::parse`, so a fixture that is not admissible fails loudly at
//! construction. Candidates ground every evidence item on the fixture repository
//! identity and cover all nine blast lanes, so they clear `Candidate::parse`.
#![allow(
    dead_code,
    unreachable_pub,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::missing_panics_doc,
    clippy::must_use_candidate
)]

use std::path::Path;

use revy_plan::document::ParsedPlan;
use revy_plan::model::review::Finding;
use revy_plan::packet::{RawWorkResult, WorkKind};
use revy_plan::render::{self, RenderContext, RenderedPlan};
use revy_plan::state::{ReviewRound, RunState};
use revy_plan::workflow::{ApplyError, apply, frontier};
use serde_json::{Value, json};

pub const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const REVISION: &str = "rev1";

/// A repository the fixtures are bound to. Evidence targets match this identity.
pub fn repository() -> Value {
    json!({
        "root": "/tmp/fixture-repo",
        "name": "fixture-repo-000000000000",
        "revision": REVISION,
        "dirty": { "changed_paths": 0, "digest": DIGEST },
        "captured_at": "2026-08-26T00:00:00Z"
    })
}

/// A capability report with one built-in capability and one proposal (`serena`).
pub fn report() -> Value {
    json!({
        "repository": { "revision": REVISION, "dirty_digest": DIGEST },
        "capabilities": [
            { "id": "capability-fs", "name": "filesystem", "version": null,
              "interface": "read", "proves": ["textual"], "limitations": [] }
        ],
        "proposals": [
            { "id": "proposal-serena", "name": "serena", "closes": ["uncertainty-consumers"],
              "benefit": "Resolve call edges.", "cost": "Downloads a server.",
              "side_effects": [{"kind": "download"}], "recommendation": "add_now" }
        ]
    })
}

/// The report digest, matching `report()`.
pub fn report_digest() -> String {
    use revy_plan::capability::CapabilityReport;
    let report: CapabilityReport = serde_json::from_value(report()).expect("report parses");
    revy_plan::state::report_digest(&report).as_str().to_owned()
}

/// One grounded documentation evidence item — no filesystem anchor, so it clears
/// the runtime check without any real files on disk.
fn grounded_doc_evidence(id: &str) -> Value {
    json!({
        "kind": "documentation", "id": id, "document": "README.md", "section": "Queue",
        "observation": "The queue is bounded.",
        "target": { "revision": REVISION, "dirty_digest": DIGEST }, "limitation": null
    })
}

/// The nine-lane coverage checklist, covering `queue` under `covered_lane` and
/// setting every other lane aside.
fn coverage(covered_lane: &str) -> Value {
    let lanes = [
        "api",
        "data_at_rest",
        "concurrency",
        "resource_bounds",
        "observability",
        "compatibility",
        "configuration",
        "tests",
        "security",
    ];
    let entries: Vec<Value> = lanes
        .iter()
        .map(|lane| {
            if *lane == covered_lane {
                json!({ "lane": lane, "coverage": { "state": "covered", "subjects": ["queue"] } })
            } else {
                json!({ "lane": lane, "coverage": { "state": "not_considered", "reason": "No change." } })
            }
        })
        .collect();
    Value::Array(entries)
}

/// The lanes set aside as `not_considered` for `covered_lane` — exactly what an
/// integrated review's `lanes` must cover.
pub fn not_considered_lanes(covered_lane: &str) -> Vec<&'static str> {
    [
        "api",
        "data_at_rest",
        "concurrency",
        "resource_bounds",
        "observability",
        "compatibility",
        "configuration",
        "tests",
        "security",
    ]
    .into_iter()
    .filter(|lane| *lane != covered_lane)
    .collect()
}

/// A confirmed lane verdict for every not-considered lane — the coverage an
/// integrated review over a `covered_lane` candidate must return.
pub fn confirmed_lane_verdicts(covered_lane: &str) -> Value {
    let verdicts: Vec<Value> = not_considered_lanes(covered_lane)
        .into_iter()
        .map(|lane| json!({ "lane": lane, "verdict": { "kind": "confirmed" } }))
        .collect();
    Value::Array(verdicts)
}

/// A coherent candidate covering `queue` under `covered_lane`, with a `changed`
/// symbol whose kind is `symbol_change` (`added` needs no consumer check).
pub fn candidate(covered_lane: &str, symbol_change: &str) -> Value {
    json!({
        "brief": {
            "title": "Bounded queue", "summary": "Add a bounded queue",
            "scope": "the queue module", "out_of_scope": "everything else"
        },
        "decisions": [],
        "exploration": {
            "evidence": [grounded_doc_evidence("evidence-queue")],
            "claims": [{
                "id": "claim-queue", "statement": "The queue is bounded.",
                "evidence": ["evidence-queue"], "consequences": ["Producers block when full."],
                "conclusion": { "state": "established" }
            }],
            "uncertainties": [],
            "impact": [{
                "subject": "queue",
                "entry": { "state": "affected", "rationale": "Callers change.",
                           "shape": "behavioral", "claims": ["claim-queue"] }
            }],
            "coverage": coverage(covered_lane)
        },
        "content": {
            "current_behavior": "Work runs inline.",
            "target_behavior": "Work runs on a bounded queue.",
            "changed_symbols": [{ "symbol": "push", "path": "src/queue.rs", "change": symbol_change }],
            "risks": [],
            "orientation": null,
            "sections": [{
                "heading": "Overview",
                "blocks": [{ "kind": "prose", "text": "The queue exposes a bounded push/pop." }]
            }]
        },
        "delivery": {
            "requirements": [],
            "acceptance_criteria": [{
                "id": "ac-1", "statement": "Push blocks when the queue is full.",
                "verify": { "kind": "manual", "procedure": "Fill the queue, then push." }
            }],
            "mrs": [{
                "id": "mr-1", "title": "Add the bounded queue",
                "files": ["src/queue.rs"], "needs": [], "satisfies": ["ac-1"],
                "standalone_behavior": "Adds a bounded queue behind a mutex.", "verify": null
            }]
        }
    })
}

/// The default integrated-only candidate (`queue` covered under `tests`, an
/// `added` symbol).
pub fn simple_candidate() -> Value {
    candidate("tests", "added")
}

/// A candidate whose `queue` behavioral change under `concurrency` triggers the
/// concurrency specialist.
pub fn concurrency_candidate() -> Value {
    candidate("concurrency", "added")
}

/// A `Drafting` document whose brief, decisions, and exploration are taken from
/// `candidate`; applying `draft_result(candidate)` re-assembles that candidate and
/// derives its reviewer roster.
pub fn drafting_document(candidate: &Value) -> Value {
    document(
        "drafting",
        json!({
            "capabilities": { "report": report(), "approved": [] },
            "context": candidate["exploration"].clone(),
            "brief": candidate["brief"].clone(),
            "decisions": candidate["decisions"].clone()
        }),
    )
}

/// The draft result carrying `candidate`'s content and delivery.
pub fn draft_result(candidate: &Value) -> Value {
    json!({ "content": candidate["content"].clone(), "delivery": candidate["delivery"].clone() })
}

/// Wrap a run-state variant into a full plan document.
pub fn document(state: &str, data: Value) -> Value {
    json!({
        "slug": "add-store-metrics",
        "repository": repository(),
        "run": { "state": state, "data": data }
    })
}

/// A `Reviewing` document over `candidate` at wave 1 with the given pending kinds.
pub fn reviewing_document(candidate: Value, pending: Value) -> Value {
    document(
        "reviewing",
        json!({ "candidate": candidate, "wave": 1, "pending": pending,
                "completed": [], "history": [] }),
    )
}

/// A clean integrated review over a `covered_lane` candidate with no findings.
pub fn clean_integrated_review(covered_lane: &str) -> Value {
    json!({
        "kind": "integrated", "findings": [],
        "lanes": confirmed_lane_verdicts(covered_lane), "consumers": []
    })
}

/// An integrated review carrying one blocking finding on the brief region.
pub fn blocking_integrated_review(covered_lane: &str, finding_id: &str) -> Value {
    json!({
        "kind": "integrated",
        "findings": [{
            "id": finding_id, "lens": "fact_check", "severity": "blocking",
            "statement": "The brief overstates the guarantee.",
            "regions": [{ "region": "brief" }]
        }],
        "lanes": confirmed_lane_verdicts(covered_lane), "consumers": []
    })
}

/// The minimal plan: awaiting the user's decision over the capability report.
pub fn minimal_plan_document() -> Value {
    document(
        "awaiting_capability_approval",
        json!({ "report": report(), "prior": null }),
    )
}

/// A final plan cleared on the merits — a candidate with one narrative block, so
/// the serve comment affordance can anchor to `s0b0`.
pub fn final_plan_document() -> Value {
    let round = json!({
        "wave": 1, "reviews": [clean_integrated_review("tests")], "triage": [], "rulings": []
    });
    document(
        "final",
        json!({ "candidate": simple_candidate(), "history": [round],
                "accepted": [], "comments": [] }),
    )
}

/// A final plan the user accepted over one unresolved blocking finding — the
/// deliverable that must show its accepted defects (design §12, §18.1).
pub fn final_accepted_document() -> Value {
    let finding = json!({
        "id": "finding-1", "lens": "fact_check", "severity": "blocking",
        "statement": "The queue can deadlock under contention.", "regions": [{ "region": "brief" }]
    });
    let round = json!({
        "wave": 3,
        "reviews": [{ "kind": "integrated", "findings": [finding], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": [{ "finding": "finding-1", "verdict": "accept" }]
    });
    document(
        "final",
        json!({ "candidate": simple_candidate(), "history": [round],
                "accepted": [finding], "comments": [] }),
    )
}

/// Parse a document, panicking with the pointer on failure.
pub fn parse(document: &Value) -> ParsedPlan {
    match ParsedPlan::parse(document.to_string().as_bytes()) {
        Ok(plan) => plan,
        Err(error) => panic!("fixture did not parse: {error}"),
    }
}

/// Try to parse a document, returning the error for negative tests.
pub fn try_parse(document: &Value) -> Result<ParsedPlan, revy_plan::document::ParsePlanError> {
    ParsedPlan::parse(document.to_string().as_bytes())
}

/// Why a workflow step failed.
#[derive(Debug)]
pub enum StepError {
    NoPacket,
    Decode(String),
    Check(String),
    Apply(Box<ApplyError>),
}

impl StepError {
    /// The apply reason, panicking if the failure was earlier in the pipeline.
    pub fn reason(&self) -> &revy_plan::workflow::ApplyReason {
        match self {
            Self::Apply(error) => error.reason(),
            other => panic!("expected an apply rejection, got {other:?}"),
        }
    }
}

/// Decode, check, and apply `result` under the packet of `kind`, in a scratch
/// transcripts home. Documentation-evidence candidates need no real files.
pub fn step(plan: ParsedPlan, kind: WorkKind, result: &Value) -> Result<ParsedPlan, StepError> {
    let scratch = std::env::temp_dir();
    step_in(plan, kind, result, &scratch)
}

pub fn step_in(
    plan: ParsedPlan,
    kind: WorkKind,
    result: &Value,
    transcripts: &Path,
) -> Result<ParsedPlan, StepError> {
    let (key, raw): (_, RawWorkResult) = {
        let frontier = frontier(&plan);
        let Some(packet) = frontier.iter().find(|packet| packet.kind() == kind) else {
            return Err(StepError::NoPacket);
        };
        let raw = packet
            .decode_result(result.to_string().as_bytes())
            .map_err(|error| StepError::Decode(error.to_string()))?;
        (packet.key().clone(), raw)
    };
    let checked = revy_plan::runtime::check_result(&plan, transcripts, raw)
        .map_err(|error| StepError::Check(error.to_string()))?;
    apply(plan, &key, checked).map_err(|error| StepError::Apply(Box::new(error)))
}

/// Decode `result` under the packet of `kind` and run only the runtime check,
/// returning the typed [`CheckError`] — for the anchor, transcript-digest, and
/// approved-tool guardrails (design §12) that `StepError::Check` erases to a
/// string.
pub fn check_step(
    plan: &ParsedPlan,
    kind: WorkKind,
    result: &Value,
    transcripts: &Path,
) -> Result<(), revy_plan::runtime::CheckError> {
    let frontier = frontier(plan);
    let packet = frontier
        .iter()
        .find(|packet| packet.kind() == kind)
        .expect("a packet of the requested kind");
    let raw = packet
        .decode_result(result.to_string().as_bytes())
        .expect("the result decodes");
    revy_plan::runtime::check_result(plan, transcripts, raw).map(|_checked| ())
}

/// The single most-common step: apply and unwrap.
pub fn apply_ok(plan: ParsedPlan, kind: WorkKind, result: &Value) -> ParsedPlan {
    match step(plan, kind, result) {
        Ok(plan) => plan,
        Err(error) => panic!("step failed: {error:?}"),
    }
}

fn state_label(state: &RunState) -> String {
    match state {
        RunState::Reviewing { wave, .. } => format!("reviewing · wave {}", wave.get()),
        RunState::Correcting { wave, .. } => format!("correcting · wave {}", wave.get()),
        RunState::AwaitingReviewDecision { wave, .. } => {
            format!("awaiting review decision · wave {}", wave.get())
        }
        RunState::Final { .. } => "final".to_owned(),
        _ => "draft".to_owned(),
    }
}

fn state_history(state: &RunState) -> &[ReviewRound] {
    match state {
        RunState::Reviewing { history, .. }
        | RunState::Correcting { history, .. }
        | RunState::AwaitingReviewDecision { history, .. }
        | RunState::Final { history, .. } => history,
        _ => &[],
    }
}

fn state_accepted(state: &RunState) -> &[Finding] {
    match state {
        RunState::Final { accepted, .. } => accepted,
        _ => &[],
    }
}

/// Render a candidate-bearing document into Markdown and HTML through the real
/// renderer, deriving the render context from the run state.
pub fn render_document(document: &Value) -> RenderedPlan {
    let plan = parse(document);
    let state = plan.state();
    let candidate = state.candidate().expect("a candidate-bearing state");
    let label = state_label(state);
    let context = RenderContext {
        state_label: &label,
        is_final: matches!(state, RunState::Final { .. }),
        history: state_history(state),
        accepted: state_accepted(state),
        comments: state.comments(),
    };
    render::render(candidate, &context)
}
