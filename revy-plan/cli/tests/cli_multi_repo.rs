//! The multi-repository command surface over two real git checkouts: `init
//! --repo`, `alias:path` citations stamped from the cited checkout, undeclared
//! aliases rejected by name, and `reopen exploration --repo`.
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

fn git(root: &Path, args: &[&str]) -> String {
    let output = StdCommand::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn seeded_repo(root: &Path, file: &str, contents: &str) {
    std::fs::create_dir(root).expect("create repo");
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "test@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::write(root.join(file), contents).expect("seed");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "init"]);
}

struct Workspace {
    temp: TempDir,
    home: PathBuf,
    dashboard: PathBuf,
    vortex: PathBuf,
}

impl Workspace {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let dashboard = temp.path().join("dashboard");
        let vortex = temp.path().join("vortex");
        std::fs::create_dir(&home).expect("create home");
        seeded_repo(&dashboard, "app.rs", "fn main() {}\n");
        seeded_repo(&vortex, "engine.rs", "fn run() {}\nfn stop() {}\n");
        Self {
            temp,
            home,
            dashboard,
            vortex,
        }
    }

    /// The CLI, run from inside the primary repository.
    fn revy(&self) -> Command {
        let mut command = Command::cargo_bin("revy-plan").expect("binary builds");
        command
            .env("REVY_PLAN_HOME", &self.home)
            .current_dir(&self.dashboard);
        command
    }

    fn capabilities(&self) -> PathBuf {
        let caps = self.home.join("capabilities.json");
        let report = json!({
            "capabilities": [{ "id": "capability-fs", "name": "filesystem", "version": null,
                               "interface": "read", "proves": ["textual"], "limitations": [] }],
            "proposals": []
        });
        std::fs::write(&caps, report.to_string()).expect("write report");
        caps
    }

    fn init(&self, extra: &[&str]) -> std::process::Output {
        self.revy()
            .args(["init", "--slug", "span-repos", "--target"])
            .arg(&self.dashboard)
            .arg("--capabilities")
            .arg(self.capabilities())
            .args(extra)
            .output()
            .expect("init runs")
    }

    fn vortex_flag(&self) -> String {
        format!("vortex={}", self.vortex.display())
    }

    fn status(&self) -> Value {
        let output = self.revy().arg("status").output().expect("status runs");
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        serde_json::from_slice(&output.stdout).expect("status json")
    }

    /// `init` declaring vortex, then approve the current tools.
    fn exploring(&self) -> PathBuf {
        let output = self.init(&["--repo", &self.vortex_flag()]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let plan = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
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

fn exploration_citing(scope: &str, path: &str) -> Value {
    json!({
        "evidence": [{
            "kind": "textual", "id": "evidence-run", "scope": [scope], "pattern": "fn run",
            "matches": [{ "location": { "path": path, "line": 1, "column": 1 }, "line": "fn run() {}" }],
            "completeness": { "kind": "complete" }, "target": null, "limitation": null
        }],
        "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "propose_capability", "report": {
            "capabilities": [], "proposals": [],
            "repository": { "revision": "r", "dirty_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
        } }
    })
}

fn plan_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("read plan")).expect("plan json")
}

#[test]
fn init_with_a_repo_flag_records_the_declared_checkout() {
    let workspace = Workspace::new();
    let output = workspace.init(&["--repo", &workspace.vortex_flag()]);
    let plan = plan_json(Path::new(String::from_utf8(output.stdout).unwrap().trim()));

    assert_eq!(
        plan["repositories"]["vortex"]["revision"],
        json!(git(&workspace.vortex, &["rev-parse", "HEAD"]))
    );
}

#[test]
fn the_exploration_packet_lists_both_repositories_by_alias() {
    let workspace = Workspace::new();
    workspace.exploring();

    let context = workspace.status()["packets"][0]["context"].clone();

    assert_eq!(
        (
            context["primary_repository"].clone(),
            context["repositories"]["vortex"]["identity"]["revision"].clone(),
        ),
        (
            json!("dashboard"),
            json!(git(&workspace.vortex, &["rev-parse", "HEAD"])),
        )
    );
}

#[test]
fn an_alias_path_citation_is_stamped_with_the_cited_checkouts_identity() {
    let workspace = Workspace::new();
    let plan = workspace.exploring();

    workspace
        .revy()
        .args(["apply", "--result", "-"])
        .write_stdin(exploration_citing("vortex:.", "vortex:engine.rs").to_string())
        .assert()
        .success();

    assert_eq!(
        plan_json(&plan)["run"]["data"]["prior"]["evidence"][0]["target"]["revision"],
        json!(git(&workspace.vortex, &["rev-parse", "HEAD"]))
    );
}

#[test]
fn an_undeclared_alias_is_rejected_naming_the_declared_aliases() {
    let workspace = Workspace::new();
    workspace.exploring();

    workspace
        .revy()
        .args(["check", "--result", "-"])
        .write_stdin(exploration_citing("mistral:.", "mistral:engine.rs").to_string())
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "repository alias `mistral` is not declared; declared aliases: `dashboard`, `vortex`",
        ));
}

#[test]
fn an_anchor_resolves_against_the_cited_checkout_not_the_primary() {
    let workspace = Workspace::new();
    workspace.exploring();

    workspace
        .revy()
        .args(["check", "--result", "-"])
        .write_stdin(exploration_citing(".", "engine.rs").to_string())
        .assert()
        .code(1)
        .stderr(predicates::str::contains(format!(
            "names source anchor `engine.rs` that is not present under target root `{}`",
            git(&workspace.dashboard, &["rev-parse", "--show-toplevel"])
        )));
}

#[test]
fn init_alias_names_the_primary_repository() {
    let workspace = Workspace::new();
    let output = workspace.init(&["--alias", "web", "--repo", &workspace.vortex_flag()]);

    let plan = plan_json(Path::new(String::from_utf8(output.stdout).unwrap().trim()));
    assert_eq!(plan["repository_alias"], json!("web"));
}

#[test]
fn a_repo_flag_without_an_alias_is_a_usage_error() {
    let workspace = Workspace::new();

    let output = workspace.init(&["--repo", &workspace.vortex.display().to_string()]);

    assert_eq!(
        (
            output.status.code(),
            String::from_utf8(output.stderr).unwrap().trim().to_owned()
        ),
        (
            Some(2),
            format!(
                "--repo `{}`: expected ALIAS=PATH",
                workspace.vortex.display()
            )
        )
    );
}

#[test]
fn a_repo_flag_reusing_the_primary_alias_is_rejected() {
    let workspace = Workspace::new();

    let output = workspace.init(&[
        "--repo",
        &format!("dashboard={}", workspace.vortex.display()),
    ]);

    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim(),
        "repository alias `dashboard` is the primary repository's alias; choose another alias (or rename the primary with `--alias`)"
    );
}

#[test]
fn reopening_exploration_with_a_repo_flag_declares_another_repository() {
    let workspace = Workspace::new();
    let plan = workspace.exploring();
    let mistral = workspace.temp.path().join("mistral");
    seeded_repo(&mistral, "model.py", "x = 1\n");

    workspace
        .revy()
        .args([
            "reopen",
            "exploration",
            "--reason",
            "the model repo matters",
            "--repo",
        ])
        .arg(format!("mistral={}", mistral.display()))
        .assert()
        .success();

    assert_eq!(
        plan_json(&plan)["repositories"]["mistral"]["revision"],
        json!(git(&mistral, &["rev-parse", "HEAD"]))
    );
}
