//! The apply/check command surface over a real checkout: the current-plan
//! fallback, stdin results, kind-name work selection, `check` as a dry run that
//! reports every problem, and `init` stamping the capability report's identity.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use assert_cmd::Command;
use serde_json::{Value, json};
use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed");
}

struct Workspace {
    _temp: TempDir,
    home: PathBuf,
    repo: PathBuf,
}

impl Workspace {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("target-repo");
        let home = temp.path().join("home");
        std::fs::create_dir(&repo).expect("create repo");
        std::fs::create_dir(&home).expect("create home");
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.email", "test@example.com"]);
        git(&repo, &["config", "user.name", "Test"]);
        std::fs::write(repo.join("queue.rs"), "fn push() {}\nfn pop() {}\n").expect("seed");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "init"]);
        Self {
            _temp: temp,
            home,
            repo,
        }
    }

    /// The CLI, run from inside the target repository.
    fn revy(&self) -> Command {
        let mut command = Command::cargo_bin("revy-plan").expect("binary builds");
        command
            .env("REVY_PLAN_HOME", &self.home)
            .current_dir(&self.repo);
        command
    }

    /// A second git repository beside the target, with one commit.
    fn other_repo(&self) -> PathBuf {
        let other = self.home.parent().unwrap().join("other-repo");
        std::fs::create_dir(&other).expect("create other repo");
        git(&other, &["init", "-q"]);
        git(&other, &["config", "user.email", "test@example.com"]);
        git(&other, &["config", "user.name", "Test"]);
        std::fs::write(other.join("lib.rs"), "fn other() {}\n").expect("seed");
        git(&other, &["add", "-A"]);
        git(&other, &["commit", "-qm", "init"]);
        other
    }

    fn init_in(&self, repo: &Path, slug: &str) -> PathBuf {
        let caps = self.home.join("capabilities.json");
        std::fs::write(
            &caps,
            json!({ "capabilities": [{ "id": "capability-fs", "name": "filesystem", "version": null,
                                       "interface": "read", "proves": ["textual"], "limitations": [] }],
                    "proposals": [] })
            .to_string(),
        )
        .expect("write report");
        let output = self
            .revy()
            .args(["init", "--slug", slug, "--target"])
            .arg(repo)
            .arg("--capabilities")
            .arg(&caps)
            .output()
            .expect("init runs");
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
    }

    /// `init` with a capability report that omits its repository identity.
    fn init(&self) -> PathBuf {
        self.init_with(&json!({
            "capabilities": [{ "id": "capability-fs", "name": "filesystem", "version": null,
                               "interface": "read", "proves": ["textual"], "limitations": [] }],
            "proposals": []
        }))
    }

    fn init_with(&self, report: &Value) -> PathBuf {
        let caps = self.home.join("capabilities.json");
        std::fs::write(&caps, serde_json::to_vec(report).unwrap()).expect("write report");
        let output = self
            .revy()
            .args(["init", "--slug", "add-metrics", "--target"])
            .arg(&self.repo)
            .arg("--capabilities")
            .arg(&caps)
            .output()
            .expect("init runs");
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
    }

    fn status(&self) -> Value {
        let output = self.revy().arg("status").output().expect("status runs");
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        serde_json::from_slice(&output.stdout).expect("status json")
    }

    /// `init`, then approve the current tools through stdin and a kind name.
    fn exploring(&self) -> PathBuf {
        let plan = self.init();
        let digest = self.status()["packets"][0]["subject"]["digest"].clone();
        let decision = json!({ "decision": "proceed_current", "report": digest });
        self.revy()
            .args(["apply", "--work", "capability_approval", "--result", "-"])
            .write_stdin(decision.to_string())
            .assert()
            .success();
        plan
    }
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("read plan")
}

/// An exploration with a runtime defect (an absent scope) and a transition
/// defect (a claim citing absent evidence).
fn doubly_broken_exploration() -> Value {
    json!({
        "evidence": [{
            "kind": "textual", "id": "evidence-text", "scope": ["absent.rs"], "pattern": "push",
            "matches": [], "completeness": { "kind": "complete" }, "target": null, "limitation": null
        }],
        "claims": [{
            "id": "claim-1", "statement": "Bounded.", "evidence": ["evidence-missing"],
            "consequences": ["Blocks."], "conclusion": { "state": "established" }
        }],
        "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "propose_capability", "report": {
            "capabilities": [], "proposals": [],
            "repository": { "revision": "r", "dirty_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
        } }
    })
}

fn sound_exploration() -> Value {
    json!({
        "evidence": [{
            "kind": "textual", "id": "evidence-text", "scope": ["queue.rs"], "pattern": "push",
            "matches": [{ "location": { "path": "queue.rs", "line": 1, "column": 4 }, "line": "fn push() {}" }],
            "completeness": { "kind": "complete" }, "target": null, "limitation": null
        }],
        "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "propose_capability", "report": {
            "capabilities": [], "proposals": [],
            "repository": { "revision": "r", "dirty_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
        } }
    })
}

#[test]
fn status_without_a_location_reads_the_plan_init_made_current() {
    let workspace = Workspace::new();
    workspace.init();

    let status = workspace.status();

    assert_eq!(status["state"], "awaiting_capability_approval");
}

#[test]
fn a_command_without_a_location_or_a_current_plan_for_the_repository_fails_loud() {
    let workspace = Workspace::new();

    let output = workspace
        .revy()
        .arg("status")
        .output()
        .expect("status runs");

    let root = workspace.repo.canonicalize().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        (
            output.status.code(),
            stderr.starts_with(&format!(
                "no plan given and no current plan for the repository at {}",
                root.display()
            ))
        ),
        (Some(2), true),
        "{stderr}"
    );
}

#[test]
fn outside_any_repository_a_command_without_a_current_plan_names_the_global_lookup() {
    let workspace = Workspace::new();

    let output = workspace
        .revy()
        .current_dir(&workspace.home)
        .arg("status")
        .output()
        .expect("status runs");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("is not inside a git repository, and there is no global current plan"),
        "{stderr}"
    );
}

#[test]
fn a_command_in_another_repository_does_not_pick_up_this_repositorys_plan() {
    let workspace = Workspace::new();
    workspace.init();
    let other = workspace.other_repo();

    let output = workspace
        .revy()
        .current_dir(&other)
        .arg("status")
        .output()
        .expect("status runs");

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn each_repository_resolves_its_own_current_plan() {
    let workspace = Workspace::new();
    let plan = workspace.init();
    let other = workspace.other_repo();
    workspace.init_in(&other, "other-change");
    let digest = workspace.status()["packets"][0]["subject"]["digest"].clone();

    let applied = workspace
        .revy()
        .args(["apply", "--work", "capability_approval", "--result", "-"])
        .write_stdin(json!({ "decision": "proceed_current", "report": digest }).to_string())
        .output()
        .expect("apply runs");

    let stdout = String::from_utf8(applied.stdout).unwrap();
    assert!(
        stdout.contains(&format!("to {} →", plan.display())),
        "{stdout}"
    );
}

#[test]
fn outside_any_repository_the_most_recently_initialized_plan_is_current() {
    let workspace = Workspace::new();
    workspace.init();
    let other = workspace.other_repo();
    workspace.init_in(&other, "other-change");

    let status = workspace
        .revy()
        .current_dir(&workspace.home)
        .args(["status"])
        .output()
        .expect("status runs");

    assert_eq!(status.status.code(), Some(0), "{status:?}");
}

#[test]
fn target_without_a_slug_uses_that_repositorys_current_plan() {
    let workspace = Workspace::new();
    workspace.init();

    let status = workspace
        .revy()
        .current_dir(&workspace.home)
        .arg("status")
        .arg("--target")
        .arg(&workspace.repo)
        .output()
        .expect("status runs");

    assert_eq!(status.status.code(), Some(0), "{status:?}");
}

#[test]
fn apply_prints_the_kind_key_plan_and_resulting_state() {
    let workspace = Workspace::new();
    let plan = workspace.init();
    let key = workspace.status()["packets"][0]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    let digest = workspace.status()["packets"][0]["subject"]["digest"].clone();

    workspace
        .revy()
        .args(["apply", "--work", "capability_approval", "--result", "-"])
        .write_stdin(json!({ "decision": "proceed_current", "report": digest }).to_string())
        .assert()
        .success()
        .stdout(format!(
            "applied capability_approval {key} to {} → exploring\n",
            plan.display()
        ));
}

#[test]
fn init_stamps_the_captured_identity_into_the_capability_report() {
    let workspace = Workspace::new();
    let plan_path = workspace.init();

    let plan: Value = serde_json::from_slice(&read(&plan_path)).unwrap();

    assert_eq!(
        plan["run"]["data"]["report"]["repository"],
        json!({
            "revision": plan["repository"]["revision"],
            "dirty_digest": plan["repository"]["dirty"]["digest"]
        })
    );
}

#[test]
fn apply_reads_the_result_from_stdin_and_selects_the_packet_by_kind() {
    let workspace = Workspace::new();
    workspace.exploring();

    let status = workspace.status();

    assert_eq!(status["state"], "exploring");
}

#[test]
fn check_reports_a_clean_result_without_writing_the_plan() {
    let workspace = Workspace::new();
    let plan_path = workspace.exploring();
    let before = read(&plan_path);

    workspace
        .revy()
        .args(["check", "--result", "-"])
        .write_stdin(sound_exploration().to_string())
        .assert()
        .success()
        .stdout(
            "ok: the result applies cleanly; the plan would enter `awaiting_capability_approval`\n",
        );

    assert_eq!(read(&plan_path), before);
}

#[test]
fn check_lists_runtime_and_transition_problems_together_without_writing_the_plan() {
    let workspace = Workspace::new();
    let plan_path = workspace.exploring();
    let before = read(&plan_path);

    let output = workspace
        .revy()
        .args(["check", "--packet", "exploration", "--result", "-"])
        .write_stdin(doubly_broken_exploration().to_string())
        .output()
        .expect("check runs");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        (
            output.status.code(),
            stderr.lines().next(),
            read(&plan_path) == before
        ),
        (Some(1), Some("result rejected (2 problems):"), true),
        "{stderr}"
    );
}

#[test]
fn apply_reports_the_same_problem_list_as_check() {
    let workspace = Workspace::new();
    workspace.exploring();
    let broken = doubly_broken_exploration().to_string();
    let checked = workspace
        .revy()
        .args(["check", "--result", "-"])
        .write_stdin(broken.clone())
        .output()
        .expect("check runs");

    let applied = workspace
        .revy()
        .args(["apply", "--result", "-"])
        .write_stdin(broken)
        .output()
        .expect("apply runs");

    assert_eq!(applied.stderr, checked.stderr);
}

#[test]
fn a_decode_failure_names_the_pointer_it_failed_at() {
    let workspace = Workspace::new();
    workspace.exploring();
    let mut result = sound_exploration();
    result["evidence"][0]["scope"] = json!(["./queue.rs"]);

    workspace
        .revy()
        .args(["check", "--result", "-"])
        .write_stdin(result.to_string())
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "result does not satisfy the `exploration` schema at /evidence/0:",
        ));
}

#[test]
fn the_exploration_packet_exposes_the_target_identity_to_copy() {
    let workspace = Workspace::new();
    let plan_path = workspace.exploring();
    let plan: Value = serde_json::from_slice(&read(&plan_path)).unwrap();

    let status = workspace.status();

    assert_eq!(
        status["packets"][0]["context"]["target_identity"]["revision"],
        plan["repository"]["revision"]
    );
}

fn lsp_report(proposals: &Value) -> Value {
    json!({
        "capabilities": [{ "id": "capability-lsp", "name": "rust-analyzer", "version": null,
                           "interface": "lsp", "proves": ["semantic"], "limitations": [] }],
        "proposals": proposals
    })
}

#[test]
fn init_with_code_intelligence_and_nothing_to_add_starts_exploring_auto_approved() {
    let workspace = Workspace::new();
    workspace.init_with(&lsp_report(&json!([])));

    let status = workspace.status();

    assert_eq!(
        (
            status["state"].clone(),
            status["capability_approval"].clone()
        ),
        (json!("exploring"), json!("auto"))
    );
}

#[test]
fn init_with_a_proposal_waits_for_the_user() {
    let workspace = Workspace::new();
    workspace.init_with(&lsp_report(&json!([{
        "id": "proposal-ast-grep", "name": "ast-grep", "closes": ["uncertainty-shape"],
        "benefit": "Structural search.", "cost": "Downloads a binary.",
        "side_effects": [{ "kind": "download" }], "recommendation": "optional"
    }])));

    let status = workspace.status();

    assert_eq!(status["state"], json!("awaiting_capability_approval"));
}

#[test]
fn init_without_code_intelligence_waits_for_the_user() {
    let workspace = Workspace::new();
    workspace.init();

    let status = workspace.status();

    assert_eq!(status["state"], json!("awaiting_capability_approval"));
}

#[test]
fn a_user_approval_is_reported_as_user_in_status() {
    let workspace = Workspace::new();
    workspace.exploring();

    let status = workspace.status();

    assert_eq!(status["capability_approval"], json!("user"));
}

impl Workspace {
    /// Exploring with `sound_exploration`'s evidence carried as the prior,
    /// stamped against the checkout as `init` captured it.
    fn exploring_with_prior(&self) -> PathBuf {
        let plan = self.exploring();
        self.revy()
            .args(["apply", "--result", "-"])
            .write_stdin(sound_exploration().to_string())
            .assert()
            .success();
        let digest = self.status()["packets"][0]["subject"]["digest"].clone();
        self.revy()
            .args(["apply", "--work", "capability_approval", "--result", "-"])
            .write_stdin(json!({ "decision": "proceed_current", "report": digest }).to_string())
            .assert()
            .success();
        plan
    }

    /// Commit a new file, moving the checkout off the captured revision.
    fn move_checkout(&self) -> String {
        std::fs::write(self.repo.join("notes.md"), "moved\n").expect("write");
        git(&self.repo, &["add", "-A"]);
        git(&self.repo, &["commit", "-qm", "move"]);
        let output = StdCommand::new("git")
            .current_dir(&self.repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
}

/// Carry the prior evidence forward, asking for a capability so the result
/// needs no complete coverage.
fn carry_prior_patch() -> Value {
    json!([{ "op": "replace", "path": "/next", "value": sound_exploration()["next"] }])
}

#[test]
fn a_moved_checkout_suggests_reopening_exploration_with_recapture() {
    let ws = Workspace::new();
    ws.exploring();
    ws.move_checkout();

    let output = ws
        .revy()
        .args(["apply", "--result", "-"])
        .write_stdin(sound_exploration().to_string())
        .output()
        .expect("apply runs");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("run `revy-plan reopen exploration --recapture --reason TEXT`")
            && !stderr.contains("set each `target` explicitly"),
        "{stderr}"
    );
}

#[test]
fn a_moved_checkout_is_not_also_reported_as_ungrounded() {
    let ws = Workspace::new();
    ws.exploring();
    ws.move_checkout();

    let output = ws
        .revy()
        .args(["apply", "--result", "-"])
        .write_stdin(sound_exploration().to_string())
        .output()
        .expect("apply runs");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("result rejected (1 problem):"),
        "{stderr}"
    );
}

#[test]
fn reopen_exploration_with_recapture_restamps_carried_evidence_on_the_moved_checkout() {
    let ws = Workspace::new();
    let plan = ws.exploring_with_prior();
    let head = ws.move_checkout();
    ws.revy()
        .args([
            "reopen",
            "exploration",
            "--recapture",
            "--reason",
            "The checkout moved.",
        ])
        .assert()
        .success();

    ws.revy()
        .args(["apply", "--patch", "-"])
        .write_stdin(carry_prior_patch().to_string())
        .assert()
        .success();

    let written: Value = serde_json::from_slice(&read(&plan)).unwrap();
    assert_eq!(
        written["run"]["data"]["prior"]["evidence"][0]["target"]["revision"],
        json!(head)
    );
}

#[test]
fn reopen_exploration_with_recapture_records_the_new_identity() {
    let ws = Workspace::new();
    ws.exploring_with_prior();
    let head = ws.move_checkout();

    ws.revy()
        .args([
            "reopen",
            "exploration",
            "--recapture",
            "--reason",
            "The checkout moved.",
        ])
        .assert()
        .success();

    assert_eq!(
        ws.status()["packets"][0]["context"]["target_identity"]["revision"],
        json!(head)
    );
}
