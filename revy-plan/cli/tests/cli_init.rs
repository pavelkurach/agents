//! `init` creates a plan awaiting capability approval from a supplied report
//! (design §11).
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

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn git(root: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed");
}

struct TargetRepo {
    _temp: TempDir,
    root: PathBuf,
}

fn target_repo() -> TargetRepo {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("target-repo");
    std::fs::create_dir(&root).expect("create target dir");
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.email", "test@example.com"]);
    git(&root, &["config", "user.name", "Test"]);
    std::fs::write(root.join("file.txt"), b"content").expect("seed file");
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "init"]);
    TargetRepo { _temp: temp, root }
}

fn capabilities_file(dir: &Path) -> PathBuf {
    let report = json!({
        "repository": { "revision": "rev1", "dirty_digest": DIGEST },
        "capabilities": [{ "id": "capability-fs", "name": "filesystem", "version": null,
                           "interface": "read", "proves": ["textual"], "limitations": [] }],
        "proposals": []
    });
    let path = dir.join("capabilities.json");
    std::fs::write(&path, serde_json::to_vec(&report).unwrap()).expect("write report");
    path
}

fn run_init(home: &Path, target: &Path, slug: &str, capabilities: &Path) -> std::process::Output {
    Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .env("REVY_PLAN_HOME", home)
        .args(["init", "--target"])
        .arg(target)
        .args(["--slug", slug, "--capabilities"])
        .arg(capabilities)
        .output()
        .expect("init runs")
}

#[test]
fn init_writes_a_plan_awaiting_capability_approval() {
    let repo = target_repo();
    let home = tempfile::tempdir().expect("plan home");
    let caps = capabilities_file(home.path());
    let output = run_init(home.path(), &repo.root, "add-metrics", &caps);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let plan: Value =
        serde_json::from_slice(&std::fs::read(PathBuf::from(stdout.trim())).unwrap()).unwrap();

    assert_eq!(plan["run"]["state"], "awaiting_capability_approval");
    assert_eq!(plan["run"]["data"]["prior"], Value::Null);
    let mut keys: Vec<&str> = plan
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["repository", "run", "slug"]);
    assert_eq!(plan["slug"], "add-metrics");
}

#[test]
fn init_requires_a_capability_report() {
    let repo = target_repo();
    let home = tempfile::tempdir().expect("plan home");
    // No --capabilities: clap must reject the invocation.
    Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .env("REVY_PLAN_HOME", home.path())
        .args(["init", "--target"])
        .arg(&repo.root)
        .args(["--slug", "add-metrics"])
        .assert()
        .code(2);
}

#[test]
fn init_refuses_to_clobber_an_existing_plan() {
    let repo = target_repo();
    let home = tempfile::tempdir().expect("plan home");
    let caps = capabilities_file(home.path());
    assert_eq!(
        run_init(home.path(), &repo.root, "add-metrics", &caps)
            .status
            .code(),
        Some(0)
    );

    Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .env("REVY_PLAN_HOME", home.path())
        .args(["init", "--target"])
        .arg(&repo.root)
        .args(["--slug", "add-metrics", "--capabilities"])
        .arg(&caps)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("already exists"));
}

#[test]
fn init_rejects_an_invalid_slug() {
    let repo = target_repo();
    let home = tempfile::tempdir().expect("plan home");
    let caps = capabilities_file(home.path());
    assert_eq!(
        run_init(home.path(), &repo.root, "Not A Slug", &caps)
            .status
            .code(),
        Some(2)
    );
}
