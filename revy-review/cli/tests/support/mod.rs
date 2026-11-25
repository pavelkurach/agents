//! Shared fixtures for the `revy-review` commands:
//! a temp git repository with one base..head change, a review directory, a skill
//! directory with rules, and a home directory for the plug-in folder `~/.revy-review/`.
#![allow(dead_code, unreachable_pub)]

use std::path::Path;
use std::process::Command as StdCommand;

use assert_cmd::Command;
use serde_json::{Value, json};

pub const CALC_BASE: &str = "pub fn total(items: &[u32]) -> u32 {\n    items.iter().sum()\n}\n";
pub const CALC_HEAD: &str =
    "pub fn total(items: &[u32]) -> u32 {\n    items.iter().take(items.len() - 1).sum()\n}\n";
pub const UTIL_HEAD: &str = "def first(xs):\n    return xs[0]\n";

pub const EDGE_RULE: &str = "# edge-of-range-change\nlangs: any\n\nWhen: The diff changes a comparison or range bound.\nAsk: Evaluate at empty, one and the bound.\nReport as: at `<edge>` the bound is wrong; fix: the bound plus a test.\n";
pub const ARC_RULE: &str = "# arc-mutex-single-owner\nlangs: rust\n\nWhen: The diff wraps state in `Arc<Mutex<_>>`.\nAsk: Does one task own all writes?\nReport as: `<state>` is shared for nothing; fix: move it into the task.\n";
pub const GO_RULE: &str = "# defer-in-loop\nlangs: go\n\nWhen: The diff adds a `defer` inside a loop.\nAsk: Does each iteration release its resource?\nReport as: `<resource>` leaks until return; fix: a function per iteration.\n";

#[derive(Debug)]
pub struct Fixture {
    pub repo: tempfile::TempDir,
    pub dir: tempfile::TempDir,
    /// The skill directory; its `rules/` holds the fixture's rules.
    pub skill: tempfile::TempDir,
    /// `HOME` of every command run, so the owner's real `~/.revy-review/` stays out.
    pub home: tempfile::TempDir,
    pub base: String,
    pub head: String,
}

pub fn git(repo: &Path, args: &[&str]) -> String {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

pub fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, content).expect("write");
}

pub fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative)).expect("read")
}

/// base: `src/calc.rs`, `README.md`; head: `src/calc.rs` changed on line 2 and
/// `src/util.py` added. The review directory holds `state.json` for that range
/// and the fixture's skill directory.
pub fn fixture() -> Fixture {
    let repo = tempfile::tempdir().expect("tempdir");
    let root = repo.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "T"]);
    write(root, "src/calc.rs", CALC_BASE);
    write(root, "README.md", "hello\n");
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "base"]);
    let base = git(root, &["rev-parse", "HEAD"]);
    write(root, "src/calc.rs", CALC_HEAD);
    write(root, "src/util.py", UTIL_HEAD);
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "head"]);
    let head = git(root, &["rev-parse", "HEAD"]);

    let skill = tempfile::tempdir().expect("tempdir");
    write(
        skill.path(),
        "rules/logic/edge-of-range-change.md",
        EDGE_RULE,
    );
    write(
        skill.path(),
        "rules/rust/arc-mutex-single-owner.md",
        ARC_RULE,
    );
    write(skill.path(), "rules/go/defer-in-loop.md", GO_RULE);

    let fixture = Fixture {
        repo,
        dir: tempfile::tempdir().expect("tempdir"),
        skill,
        home: tempfile::tempdir().expect("tempdir"),
        base,
        head,
    };
    fixture.write_state(&json!({
        "repo": fixture.repo.path(),
        "target": "feature",
        "base": fixture.base,
        "head": fixture.head,
        "mode": "team",
        "round": 1,
        "skill": fixture.skill.path(),
    }));
    fixture
}

impl Fixture {
    /// The skill's rules directory.
    pub fn rules(&self) -> std::path::PathBuf {
        self.skill.path().join("rules")
    }

    /// The plug-in folder, `~/.revy-review/`; absent until a test writes into it.
    pub fn plugin(&self) -> std::path::PathBuf {
        self.home.path().join(".revy-review")
    }

    pub fn write_state(&self, state: &Value) {
        self.write_json("state.json", state);
    }

    /// Records the coordinator's specialist decisions in `state.json`.
    pub fn record_specialists(&self, decisions: &Value) {
        let mut state: Value =
            serde_json::from_str(&read(self.dir.path(), "state.json")).expect("state.json");
        state["specialists"] = decisions.clone();
        self.write_state(&state);
    }

    pub fn write_json(&self, relative: &str, value: &Value) {
        write(
            self.dir.path(),
            relative,
            &serde_json::to_string_pretty(value).expect("json"),
        );
    }

    /// Built-in reviewers 2..=4, each covering every changed file and finding nothing;
    /// reviewer 1 is left to the test.
    pub fn write_empty_reviews_2_to_4(&self) {
        for n in 2..=4 {
            self.write_json(&format!("reviews/{n}.json"), &empty_review(n));
        }
    }

    /// `revy-review` run from the review directory.
    pub fn revy_review(&self) -> Command {
        let mut command = Command::cargo_bin("revy-review").expect("binary `revy-review` builds");
        command
            .current_dir(self.dir.path())
            .env("HOME", self.home.path());
        command
    }
}

pub fn valid_problem() -> Value {
    json!({
        "statement": "Invoices total every line item.",
        "source": "ticket",
        "criteria": [
            {"id": "C1", "text": "total sums every item", "quote": "Totals must include every line",
             "implied": false, "verdict": "not_met", "evidence": ["src/calc.rs:2"]},
            {"id": "C2", "text": "helpers stay small", "quote": null, "implied": true,
             "verdict": "met", "evidence": ["src/util.py:1"]}
        ],
        "out_of_scope": ["currency rounding"],
        "scope_creep": [{"id": "S1", "sites": ["src/util.py:1"], "why": "first() serves no criterion"}],
        "blocks": true
    })
}

pub fn valid_review() -> Value {
    json!({
        "reviewer": 1,
        "lens": null,
        "ledger": [
            {"kind": "contract", "subject": "total sums all items", "site": "src/calc.rs:2",
             "verdict": "finding", "finding": "1.1"},
            {"kind": "contract", "subject": "first returns the head of the list", "site": "src/util.py:2",
             "verdict": "fine", "finding": null}
        ],
        "findings": [
            {"id": "1.1", "title": "total drops the last item", "severity": "blocking",
             "sites": ["src/calc.rs:2"], "quote": "items.iter().take(items.len() - 1).sum()",
             "breaks": "total(&[1, 2]) returns 1.", "fix": "Sum every item.",
             "rule": "edge-of-range-change"}
        ]
    })
}

/// A reviewer file that covers every changed file and found nothing.
pub fn empty_review(reviewer: u64) -> Value {
    json!({
        "reviewer": reviewer,
        "lens": null,
        "ledger": [
            {"kind": "contract", "subject": "total keeps its signature", "site": "src/calc.rs:1",
             "verdict": "fine", "finding": null},
            {"kind": "contract", "subject": "first returns the head of the list", "site": "src/util.py:1",
             "verdict": "fine", "finding": null}
        ],
        "findings": []
    })
}

/// Reviewer 1 with findings 1.1 and 1.2, the ones `valid_verified` keeps.
pub fn review_with_two_findings() -> Value {
    let mut review = valid_review();
    review["findings"]
        .as_array_mut()
        .expect("findings")
        .push(json!(
            {"id": "1.2", "title": "first panics on an empty list", "severity": "question",
             "sites": ["src/util.py:2"], "quote": "return xs[0]", "breaks": "first([]) raises.",
             "fix": "Return None.", "rule": null}
        ));
    review
}

pub fn valid_verified() -> Value {
    json!({
        "findings": [
            {"id": "F1", "title": "total drops the last item", "severity": "blocking",
             "sites": ["src/calc.rs:2"], "quote": "items.iter().take(items.len() - 1).sum()",
             "breaks": "total(&[1, 2]) returns 1, so every invoice is short by its last line.",
             "fix": "Sum every item: items.iter().sum().", "rule": "edge-of-range-change",
             "merged_from": ["1.1"]},
            {"id": "F2", "title": "first panics on an empty list", "severity": "question",
             "sites": ["src/util.py:2", "src/calc.rs:1"], "quote": "return xs[0]",
             "breaks": "first([]) raises IndexError.", "fix": "Return None for an empty list.",
             "rule": null, "merged_from": ["1.2"]}
        ],
        "dropped": []
    })
}
