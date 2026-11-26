//! The recovery and editing commands over a plan file: `reopen`, `revise`
//! (with its `--check` dry run), the draft `--patch` a reopen enables, and
//! `transcript add`.
#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use common::{
    apply_ok, clean_integrated_review, draft_result, drafting_document, parse, simple_candidate,
};
use revy_plan::packet::WorkKind;
use serde_json::{Value, json};
use tempfile::TempDir;

struct Home {
    dir: TempDir,
}

impl Home {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("home"),
        }
    }

    fn revy(&self) -> Command {
        let mut command = Command::cargo_bin("revy-plan").expect("binary builds");
        command.env("REVY_PLAN_HOME", self.dir.path());
        command
    }

    /// A final plan, reached by drafting and one clean wave, written to disk.
    fn final_plan(&self) -> PathBuf {
        self.final_plan_of(&simple_candidate())
    }

    fn final_plan_of(&self, candidate: &Value) -> PathBuf {
        let candidate = candidate.clone();
        let reviewing = apply_ok(
            parse(&drafting_document(&candidate)),
            WorkKind::Draft,
            &draft_result(&candidate),
        );
        let plan = apply_ok(
            reviewing,
            WorkKind::IntegratedReview,
            &clean_integrated_review("tests"),
        );
        let path = self.dir.path().join("plan.json");
        std::fs::write(&path, plan.to_canonical_json().unwrap()).expect("write plan");
        path
    }

    fn write(&self, name: &str, value: &Value) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::write(&path, value.to_string()).expect("write input");
        path
    }

    fn status(&self, plan: &Path) -> Value {
        let output = self
            .revy()
            .arg("status")
            .arg("--plan")
            .arg(plan)
            .output()
            .expect("status runs");
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        serde_json::from_slice(&output.stdout).expect("status json")
    }
}

fn summary_patch() -> Value {
    json!([{ "op": "replace", "path": "/candidate/brief/summary", "value": "Add a bounded queue, revised" }])
}

#[test]
fn reopen_draft_records_the_reason_in_status() {
    let home = Home::new();
    let plan = home.final_plan();

    home.revy()
        .args(["reopen", "draft", "--reason", "Split the MR.", "--plan"])
        .arg(&plan)
        .assert()
        .success()
        .stdout(format!(
            "applied reopen draft to {} → drafting\n",
            plan.display()
        ));

    assert_eq!(
        home.status(&plan)["reopens"],
        json!([{ "target": "draft", "reason": "Split the MR." }])
    );
}

#[test]
fn render_after_reopening_a_draft_shows_the_prior_candidate_watermarked_reopened() {
    let home = Home::new();
    let plan = home.final_plan();
    home.revy()
        .args(["reopen", "draft", "--reason", "Split the MR.", "--plan"])
        .arg(&plan)
        .assert()
        .success();
    let out = home.dir.path().join("out");

    home.revy()
        .args(["render", "--plan"])
        .arg(&plan)
        .arg("--out")
        .arg(&out)
        .assert()
        .success();

    let markdown = std::fs::read_to_string(out.join("plan.md")).unwrap();
    assert!(markdown.contains("> **DRAFT — reopened**"), "{markdown}");
}

#[test]
fn render_before_any_candidate_says_why_it_cannot_render() {
    let home = Home::new();
    let plan = home.write("drafting.json", &drafting_document(&simple_candidate()));

    home.revy()
        .args(["render", "--plan"])
        .arg(&plan)
        .assert()
        .code(1)
        .stderr("render requires a candidate: the plan is `drafting`, and no draft set aside by a reopen is retained to preview\n");
}

#[test]
fn after_reopening_a_draft_the_draft_packet_accepts_a_patch() {
    let home = Home::new();
    let plan = home.final_plan();
    home.revy()
        .args(["reopen", "draft", "--reason", "Retarget.", "--plan"])
        .arg(&plan)
        .assert()
        .success();
    let patch = home.write(
        "draft-patch.json",
        &json!([{ "op": "replace", "path": "/content/target_behavior", "value": "Work runs on a bounded, drained queue." }]),
    );

    home.revy()
        .args(["apply", "--patch"])
        .arg(&patch)
        .arg("--plan")
        .arg(&plan)
        .assert()
        .success();

    assert_eq!(home.status(&plan)["state"], json!("reviewing"));
}

#[test]
fn revise_check_reports_the_outcome_without_writing_the_plan() {
    let home = Home::new();
    let plan = home.final_plan();
    let before = std::fs::read(&plan).unwrap();
    let patch = home.write("patch.json", &summary_patch());

    home.revy()
        .args(["revise", "--check", "--note", "Tighten.", "--patch"])
        .arg(&patch)
        .arg("--plan")
        .arg(&plan)
        .assert()
        .success()
        .stdout("ok: the revision applies cleanly; the plan would enter `reviewing · wave 2`\n");

    assert_eq!(std::fs::read(&plan).unwrap(), before);
}

#[test]
fn revise_records_the_revision_and_reenters_review() {
    let home = Home::new();
    let plan = home.final_plan();
    let patch = home.write("patch.json", &summary_patch());

    home.revy()
        .args(["revise", "--note", "Tighten.", "--patch"])
        .arg(&patch)
        .arg("--plan")
        .arg(&plan)
        .assert()
        .success();

    assert_eq!(
        home.status(&plan)["latest_revision"],
        json!({ "wave": 2, "note": "Tighten.", "addresses": [] })
    );
}

#[test]
fn status_shows_the_user_decision_changes_of_the_latest_revision() {
    let home = Home::new();
    let mut candidate = simple_candidate();
    candidate["decisions"] = json!([{
        "id": "decision-bound", "owner": "user", "question": "Which bound?",
        "state": { "state": "answered", "answer": "1024",
                   "consequences": ["Producers block when full."],
                   "ruled_out": [{ "alternative": "4096", "reason": "not chosen by user" }] }
    }]);
    let plan = home.final_plan_of(&candidate);
    let patch = home.write(
        "patch.json",
        &json!([
            { "op": "replace", "path": "/candidate/decisions/@decision-bound/state/answer", "value": "4096" },
            { "op": "replace", "path": "/candidate/decisions/@decision-bound/state/ruled_out/0/alternative", "value": "1024" }
        ]),
    );

    home.revy()
        .args(["revise", "--note", "The user picked 4096.", "--patch"])
        .arg(&patch)
        .arg("--plan")
        .arg(&plan)
        .assert()
        .success();

    assert_eq!(
        home.status(&plan)["latest_revision"]["decision_changes"],
        json!([{ "decision": "decision-bound", "field": "answer", "before": "1024", "after": "4096" }])
    );
}

#[test]
fn revise_prints_the_plan_it_revised_and_the_state_it_entered() {
    let home = Home::new();
    let plan = home.final_plan();
    let patch = home.write("patch.json", &summary_patch());

    home.revy()
        .args(["revise", "--note", "Tighten.", "--patch"])
        .arg(&patch)
        .arg("--plan")
        .arg(&plan)
        .assert()
        .success()
        .stdout(format!(
            "applied revise to {} → reviewing · wave 2\n",
            plan.display()
        ));
}

#[test]
fn revise_lists_every_problem_and_exits_one() {
    let home = Home::new();
    let plan = home.final_plan();
    let patch = home.write("patch.json", &json!([]));

    let output = home
        .revy()
        .args([
            "revise",
            "--note",
            "No-op.",
            "--addresses",
            "finding-x",
            "--patch",
        ])
        .arg(&patch)
        .arg("--plan")
        .arg(&plan)
        .output()
        .expect("revise runs");

    assert_eq!(
        (output.status.code(), String::from_utf8(output.stderr).unwrap()),
        (
            Some(1),
            "result rejected (2 problems):\n  - [transition] /candidate: the revised candidate is identical to the current one; a revision must change it\n  - [transition] /addresses/0: --addresses names finding `finding-x`, which no review of this candidate raised (known: [])\n".to_owned()
        )
    );
}

#[test]
fn a_patch_selector_addresses_a_mr_by_id_in_revise() {
    let home = Home::new();
    let plan = home.final_plan();
    let patch = home.write(
        "patch.json",
        &json!([{ "op": "replace", "path": "/candidate/delivery/mrs/@mr-1/title", "value": "Add the queue" }]),
    );

    home.revy()
        .args(["revise", "--note", "Rename the MR.", "--patch"])
        .arg(&patch)
        .arg("--plan")
        .arg(&plan)
        .assert()
        .success();

    let saved: Value = serde_json::from_slice(&std::fs::read(&plan).unwrap()).unwrap();
    assert_eq!(
        saved["run"]["data"]["candidate"]["delivery"]["mrs"][0]["title"],
        json!("Add the queue")
    );
}

#[test]
fn transcript_add_stores_the_file_by_digest_and_prints_its_reference() {
    let home = Home::new();
    let file = home.dir.path().join("run.txt");
    std::fs::write(&file, "ok\n").unwrap();
    let digest = revy_plan::primitives::Sha256Digest::hash(b"ok\n");

    let output = home
        .revy()
        .args(["transcript", "add"])
        .arg(&file)
        .output()
        .expect("transcript add runs");

    let printed: Value = serde_json::from_slice(&output.stdout).expect("reference json");
    assert_eq!(
        (
            printed,
            std::fs::read(home.dir.path().join("transcripts").join(digest.as_str())).unwrap()
        ),
        (
            json!({ "path": "run.txt", "digest": digest.as_str() }),
            b"ok\n".to_vec()
        )
    );
}

#[test]
fn transcript_add_of_a_missing_file_fails_loud() {
    let home = Home::new();

    home.revy()
        .args(["transcript", "add", "/nonexistent/run.txt"])
        .assert()
        .code(2)
        .stderr(predicates::str::starts_with("/nonexistent/run.txt: "));
}
