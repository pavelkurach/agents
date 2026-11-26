//! Plans spanning several repositories: evidence cites `alias:path`, anchors
//! resolve against the cited repository's root, grounding and target stamping
//! use that repository's identity, and merge-request files and changed symbols
//! may live in any declared repository.
#![allow(
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use std::collections::BTreeMap;

use common::{
    DIGEST, REVISION, StepError, check_step_against_all, document, parse, render_document, report,
    reviewing_document, simple_candidate, step_against_all, try_parse,
};
use revy_plan::document::ParsedPlan;
use revy_plan::packet::WorkKind;
use revy_plan::primitives::RepoAlias;
use revy_plan::runtime::{Checkout, Checkouts};
use revy_plan::state::RunState;
use revy_plan::workflow::frontier;
use serde_json::{Value, json};
use tempfile::TempDir;

const VORTEX_REVISION: &str = "vrev1";

/// The primary checkout and the `vortex` checkout, each holding `src/lib.rs`.
struct Checkouts2 {
    primary: TempDir,
    vortex: TempDir,
}

impl Checkouts2 {
    fn new() -> Self {
        let primary = tempfile::tempdir().expect("primary tempdir");
        let vortex = tempfile::tempdir().expect("vortex tempdir");
        for dir in [&primary, &vortex] {
            std::fs::create_dir(dir.path().join("src")).expect("mkdir src");
            std::fs::write(dir.path().join("src/lib.rs"), "fn a() {}\nfn b() {}\n").expect("seed");
        }
        std::fs::write(vortex.path().join("src/engine.rs"), "fn run() {}\n").expect("seed");
        Self { primary, vortex }
    }

    fn root(dir: &TempDir) -> String {
        dir.path().to_str().unwrap().to_owned()
    }
}

fn vortex_identity() -> Value {
    json!({ "revision": VORTEX_REVISION, "dirty_digest": DIGEST })
}

fn primary_identity() -> Value {
    json!({ "revision": REVISION, "dirty_digest": DIGEST })
}

/// `document` declaring the `vortex` repository beside the primary, both
/// rooted in `dirs`.
fn two_repo_document(dirs: &Checkouts2, state: &str, data: Value) -> Value {
    let mut value = document(state, data);
    value["repository"]["root"] = json!(Checkouts2::root(&dirs.primary));
    value["repositories"] = json!({
        "vortex": {
            "root": Checkouts2::root(&dirs.vortex),
            "name": "vortex-000000000000",
            "revision": VORTEX_REVISION,
            "dirty": { "changed_paths": 0, "digest": DIGEST },
            "captured_at": "2026-08-26T00:00:00Z"
        }
    });
    value
}

fn exploring(dirs: &Checkouts2) -> ParsedPlan {
    parse(&two_repo_document(
        dirs,
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": [] },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        }),
    ))
}

fn definitions(paths: &[&str], target: Value) -> Value {
    let locations: Vec<Value> = paths
        .iter()
        .map(|path| json!({ "path": path, "line": 1, "column": 1 }))
        .collect();
    json!({
        "kind": "semantic", "id": "evidence-defs", "provenance": { "tool": "rust-analyzer", "version": null },
        "operation": "definition", "subject": "a",
        "observation": { "shape": "definitions", "locations": locations },
        "target": target, "limitation": null
    })
}

fn collecting(evidence: Value) -> Value {
    json!({
        "evidence": [evidence], "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "propose_capability", "report": report() }
    })
}

/// Both checkouts live and unmoved since the plan captured them.
fn unmoved() -> Checkouts {
    Checkouts::new(
        Checkout::Captured(serde_json::from_value(primary_identity()).unwrap()),
        BTreeMap::from([(
            RepoAlias::parse("vortex").unwrap(),
            Checkout::Captured(serde_json::from_value(vortex_identity()).unwrap()),
        )]),
    )
}

fn apply(plan: ParsedPlan, result: &Value, checkouts: &Checkouts) -> Result<ParsedPlan, StepError> {
    let transcripts = tempfile::tempdir().expect("transcripts tempdir");
    step_against_all(
        plan,
        WorkKind::Exploration,
        result,
        transcripts.path(),
        checkouts,
    )
}

fn rejection_text(error: &StepError) -> Vec<String> {
    match error {
        StepError::Apply(error) => error
            .reasons()
            .iter()
            .map(|r| r.error().to_string())
            .collect(),
        StepError::Check(text) => vec![text.clone()],
        other => panic!("expected a rejection, got {other:?}"),
    }
}

fn prior_target(plan: &ParsedPlan) -> Value {
    let RunState::AwaitingCapabilityApproval {
        prior: Some(prior), ..
    } = plan.state()
    else {
        panic!("expected an amendment carrying the prior context");
    };
    serde_json::to_value(prior.evidence()[0].observed_target()).unwrap()
}

// --- grounding ---------------------------------------------------------------------

#[test]
fn evidence_anchored_in_a_declared_repository_grounds_on_that_repositorys_identity() {
    let dirs = Checkouts2::new();
    let result = collecting(definitions(&["vortex:src/engine.rs"], vortex_identity()));

    let next = apply(exploring(&dirs), &result, &unmoved()).expect("grounded on vortex");

    assert_eq!(prior_target(&next), vortex_identity());
}

#[test]
fn evidence_anchored_in_a_declared_repository_but_carrying_the_primary_identity_is_stale() {
    let dirs = Checkouts2::new();
    let result = collecting(definitions(&["vortex:src/engine.rs"], primary_identity()));

    let error =
        apply(exploring(&dirs), &result, &unmoved()).expect_err("wrong repository identity");

    assert_eq!(
        rejection_text(&error),
        vec![format!(
            "exploration result is not internally consistent: STALE-MODEL: evidence `evidence-defs` was observed against revision `rev1` (dirty digest `{DIGEST}`), but the plan's repository `vortex` is at revision `vrev1` (dirty digest `{DIGEST}`)"
        )]
    );
}

#[test]
fn evidence_citing_two_repositories_under_one_target_names_both() {
    let dirs = Checkouts2::new();
    let result = collecting(definitions(
        &["src/lib.rs", "vortex:src/lib.rs"],
        primary_identity(),
    ));

    let error = apply(exploring(&dirs), &result, &unmoved()).expect_err("mixed repositories");

    assert_eq!(
        rejection_text(&error),
        vec![
            "exploration result is not internally consistent: evidence `evidence-defs` cites repositories `fixture-repo`, `vortex` but carries one `target`; split it into one evidence item per repository"
                .to_owned()
        ]
    );
}

#[test]
fn evidence_citing_an_undeclared_alias_lists_the_declared_aliases() {
    let dirs = Checkouts2::new();
    let result = collecting(definitions(&["mistral:src/lib.rs"], primary_identity()));

    let error = apply(exploring(&dirs), &result, &unmoved()).expect_err("unknown alias");

    assert_eq!(
        rejection_text(&error),
        vec![
            "exploration result is not internally consistent: evidence `evidence-defs`: repository alias `mistral` is not declared; declared aliases: `fixture-repo`, `vortex`; declare it with `revy-plan init --repo mistral=PATH` or `revy-plan reopen exploration --repo mistral=PATH`"
                .to_owned()
        ]
    );
}

#[test]
fn a_primary_path_prefixed_with_the_primary_alias_is_rejected() {
    let dirs = Checkouts2::new();
    let result = collecting(definitions(
        &["fixture-repo:src/lib.rs"],
        primary_identity(),
    ));

    let error = apply(exploring(&dirs), &result, &unmoved()).expect_err("primary prefixed");

    assert_eq!(
        rejection_text(&error),
        vec![
            "exploration result is not internally consistent: evidence `evidence-defs`: `fixture-repo:` names the primary repository, which is cited unprefixed; drop the `fixture-repo:` prefix"
                .to_owned()
        ]
    );
}

#[test]
fn anchorless_evidence_may_ground_on_a_declared_repositorys_identity() {
    let dirs = Checkouts2::new();
    let result = collecting(json!({
        "kind": "documentation", "id": "evidence-doc", "document": "vortex README", "section": "Engine",
        "observation": "The engine is single-threaded.", "target": vortex_identity(), "limitation": null
    }));

    let next = apply(exploring(&dirs), &result, &unmoved()).expect("grounded on vortex");

    assert_eq!(prior_target(&next), vortex_identity());
}

#[test]
fn anchorless_evidence_matching_no_declared_repository_lists_them() {
    let dirs = Checkouts2::new();
    let result = collecting(json!({
        "kind": "documentation", "id": "evidence-doc", "document": "README", "section": "Engine",
        "observation": "The engine is single-threaded.",
        "target": { "revision": "elsewhere", "dirty_digest": DIGEST }, "limitation": null
    }));

    let error = apply(exploring(&dirs), &result, &unmoved()).expect_err("no repository matches");

    assert_eq!(
        rejection_text(&error),
        vec![format!(
            "exploration result is not internally consistent: STALE-MODEL: evidence `evidence-doc` cites no source anchor and was observed against revision `elsewhere` (dirty digest `{DIGEST}`), which is the state of none of the plan's repositories (`fixture-repo`, `vortex`)"
        )]
    );
}

// --- anchors and stamping ------------------------------------------------------------

#[test]
fn an_anchor_absent_from_the_cited_repository_names_that_repositorys_root() {
    let dirs = Checkouts2::new();
    let plan = exploring(&dirs);
    // `src/engine.rs` exists only in vortex, so the primary does not have it.
    let result = collecting(definitions(&["src/engine.rs"], primary_identity()));
    let transcripts = tempfile::tempdir().unwrap();

    let issues = check_step_against_all(
        &plan,
        WorkKind::Exploration,
        &result,
        transcripts.path(),
        &unmoved(),
    )
    .expect_err("absent from the primary");

    assert!(
        issues.first().error().to_string().contains(&format!(
            "names source anchor `src/engine.rs` that is not present under target root `{}`",
            Checkouts2::root(&dirs.primary)
        )),
        "{}",
        issues.first().error()
    );
}

#[test]
fn a_whole_repository_textual_scope_is_accepted() {
    let dirs = Checkouts2::new();
    let result = collecting(json!({
        "kind": "textual", "id": "evidence-grep", "scope": ["vortex:."], "pattern": "unsafe",
        "matches": [], "completeness": { "kind": "complete" }, "target": vortex_identity(), "limitation": null
    }));

    let next = apply(exploring(&dirs), &result, &unmoved()).expect("whole-repo scope");

    assert_eq!(prior_target(&next), vortex_identity());
}

#[test]
fn evidence_omitting_its_target_is_stamped_with_the_cited_repositorys_identity() {
    let dirs = Checkouts2::new();
    let result = collecting(definitions(&["vortex:src/engine.rs"], Value::Null));

    let next = apply(exploring(&dirs), &result, &unmoved()).expect("stamped from vortex");

    assert_eq!(prior_target(&next), vortex_identity());
}

#[test]
fn evidence_omitting_its_target_is_rejected_when_the_cited_checkout_moved() {
    let dirs = Checkouts2::new();
    let moved = Checkouts::new(
        Checkout::Captured(serde_json::from_value(primary_identity()).unwrap()),
        BTreeMap::from([(
            RepoAlias::parse("vortex").unwrap(),
            Checkout::Captured(
                serde_json::from_value(json!({ "revision": "vrev2", "dirty_digest": DIGEST }))
                    .unwrap(),
            ),
        )]),
    );
    let result = collecting(definitions(&["vortex:src/engine.rs"], Value::Null));

    let error = apply(exploring(&dirs), &result, &moved).expect_err("vortex moved");

    assert!(
        rejection_text(&error)[0].contains(&format!(
            "evidence `evidence-defs` omits `target`, but the checkout of repository `vortex` (`{}`) moved since the plan was captured: plan revision `vrev1`",
            Checkouts2::root(&dirs.vortex)
        )),
        "{:?}",
        rejection_text(&error)
    );
}

// --- candidate paths -----------------------------------------------------------------

fn candidate_in_vortex(mr_file: &str, symbol_path: &str) -> Value {
    let mut candidate = simple_candidate();
    candidate["delivery"]["mrs"][0]["files"] = json!([mr_file]);
    candidate["content"]["changed_symbols"][0]["path"] = json!(symbol_path);
    candidate
}

#[test]
fn a_changed_symbol_in_a_declared_repository_listed_by_a_merge_request_is_coherent() {
    let dirs = Checkouts2::new();
    let candidate = candidate_in_vortex("vortex:src/engine.rs", "vortex:src/engine.rs");

    let plan = try_parse(&two_repo_document(
        &dirs,
        "reviewing",
        json!({ "candidate": candidate, "wave": 1, "pending": [{ "kind": "integrated" }], "completed": [], "history": [] }),
    ));

    assert!(plan.is_ok(), "{:?}", plan.err());
}

#[test]
fn a_changed_symbol_whose_repository_differs_from_the_merge_request_file_is_unlisted() {
    let dirs = Checkouts2::new();
    let candidate = candidate_in_vortex("src/engine.rs", "vortex:src/engine.rs");

    let error = try_parse(&two_repo_document(
        &dirs,
        "reviewing",
        json!({ "candidate": candidate, "wave": 1, "pending": [{ "kind": "integrated" }], "completed": [], "history": [] }),
    ))
    .expect_err("vortex:src/engine.rs is not src/engine.rs");

    assert_eq!(
        error.message(),
        "changed symbol `push` lives in `vortex:src/engine.rs`, which no merge request lists in its files"
    );
}

#[test]
fn a_merge_request_file_in_an_undeclared_repository_is_rejected() {
    let candidate = candidate_in_vortex("mistral:src/x.rs", "mistral:src/x.rs");

    let error = try_parse(&reviewing_document(
        candidate,
        json!([{ "kind": "integrated" }]),
    ))
    .expect_err("mistral is not declared");

    assert_eq!(
        error.pointer(),
        "/run/data/candidate/delivery/mrs/0/files/0"
    );
}

#[test]
fn a_verify_command_cwd_in_an_undeclared_repository_is_rejected() {
    let mut candidate = simple_candidate();
    candidate["delivery"]["mrs"][0]["verify"] =
        json!({ "program": "cargo", "args": ["test"], "cwd": "mistral:src", "expected_status": 0 });

    let error = try_parse(&reviewing_document(
        candidate,
        json!([{ "kind": "integrated" }]),
    ))
    .expect_err("mistral is not declared");

    assert_eq!(
        error.pointer(),
        "/run/data/candidate/delivery/mrs/0/verify/cwd"
    );
}

#[test]
fn a_verify_command_cwd_in_a_declared_repository_is_coherent() {
    let dirs = Checkouts2::new();
    let mut candidate = simple_candidate();
    candidate["delivery"]["acceptance_criteria"][0]["verify"] = json!({
        "kind": "command", "program": "cargo", "args": ["test"], "cwd": "vortex:src", "expected_status": 0
    });

    let plan = try_parse(&two_repo_document(
        &dirs,
        "reviewing",
        json!({ "candidate": candidate, "wave": 1, "pending": [{ "kind": "integrated" }], "completed": [], "history": [] }),
    ));

    assert!(plan.is_ok(), "{:?}", plan.err());
}

#[test]
fn a_tree_entry_in_an_undeclared_repository_is_rejected() {
    let mut candidate = simple_candidate();
    candidate["content"]["sections"] = json!([{ "heading": "Files", "blocks": [{ "kind": "figure",
        "diagram": { "kind": "tree", "entries": [{ "path": "mistral:src/x.rs", "change": null, "note": null }] } }] }]);

    let error = try_parse(&reviewing_document(
        candidate,
        json!([{ "kind": "integrated" }]),
    ))
    .expect_err("mistral is not declared");

    assert_eq!(
        error.pointer(),
        "/run/data/candidate/content/sections/0/blocks/0/diagram/entries/0/path"
    );
}

#[test]
fn anchorless_evidence_omitting_its_target_in_a_multi_repository_plan_is_not_stamped() {
    let dirs = Checkouts2::new();
    let result = collecting(json!({
        "kind": "semantic", "id": "evidence-defs", "provenance": { "tool": "rust-analyzer", "version": null },
        "operation": "references", "subject": "a",
        "observation": { "shape": "references", "references": [], "completeness": { "kind": "complete" } },
        "target": null, "limitation": null
    }));

    let error = apply(exploring(&dirs), &result, &unmoved()).expect_err("no repository to stamp");

    assert_eq!(
        rejection_text(&error),
        vec![
            "exploration result is not internally consistent: evidence `evidence-defs` cites no source anchor and omits `target`; in a plan with several repositories set `target` to the identity of the repository it observed (one of `fixture-repo`, `vortex`; `revy-plan status` lists each identity)".to_owned()
        ]
    );
}

// --- the plan document -----------------------------------------------------------------

#[test]
fn a_declared_repository_reusing_the_primary_alias_is_rejected_at_load() {
    let dirs = Checkouts2::new();
    let mut value = exploring_document(&dirs);
    value["repositories"] = json!({ "fixture-repo": value["repositories"]["vortex"].clone() });

    let error = try_parse(&value).expect_err("alias collides with the primary");

    assert_eq!(
        error.message(),
        "repository alias `fixture-repo` is the primary repository's alias; choose another alias (or rename the primary with `--alias`)"
    );
}

fn exploring_document(dirs: &Checkouts2) -> Value {
    two_repo_document(
        dirs,
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": [] },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        }),
    )
}

#[test]
fn a_single_repository_plan_round_trips_without_repository_fields() {
    let original = reviewing_document(simple_candidate(), json!([{ "kind": "integrated" }]));

    let canonical: Value =
        serde_json::from_str(&parse(&original).to_canonical_json().unwrap()).unwrap();

    assert_eq!(canonical, original);
}

#[test]
fn a_two_repository_plan_round_trips_its_declared_repositories() {
    let dirs = Checkouts2::new();
    let original = exploring_document(&dirs);

    let canonical: Value =
        serde_json::from_str(&parse(&original).to_canonical_json().unwrap()).unwrap();

    assert_eq!(canonical["repositories"], original["repositories"]);
}

#[test]
fn the_exploration_packet_lists_every_repository_and_how_to_cite_it() {
    let dirs = Checkouts2::new();
    let plan = exploring(&dirs);

    let context = frontier(&plan).only().unwrap().context().clone();

    assert_eq!(
        (
            context["primary_repository"].clone(),
            context["repositories"]["vortex"]["identity"].clone(),
            context["repositories"]["vortex"]["cite_as"].clone(),
            context["repositories"]["fixture-repo"]["cite_as"].clone(),
        ),
        (
            json!("fixture-repo"),
            vortex_identity(),
            json!("vortex:path"),
            json!("path"),
        )
    );
}

// --- render ------------------------------------------------------------------------------

#[test]
fn the_render_qualifies_non_primary_paths_with_their_alias() {
    let dirs = Checkouts2::new();
    let mut candidate = candidate_in_vortex("vortex:src/engine.rs", "vortex:src/engine.rs");
    candidate["delivery"]["mrs"][0]["files"] = json!(["src/queue.rs", "vortex:src/engine.rs"]);
    candidate["content"]["changed_symbols"] = json!([
        { "symbol": "push", "path": "src/queue.rs", "change": "added" },
        { "symbol": "run", "path": "vortex:src/engine.rs", "change": "modified" }
    ]);

    let rendered = render_document(&two_repo_document(
        &dirs,
        "reviewing",
        json!({ "candidate": candidate, "wave": 1, "pending": [{ "kind": "integrated" }], "completed": [], "history": [] }),
    ));

    assert!(
        rendered.markdown.contains("vortex:src/") && !rendered.markdown.contains("fixture-repo:"),
        "{}",
        rendered.markdown
    );
}
