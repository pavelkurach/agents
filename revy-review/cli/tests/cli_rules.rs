#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

mod support;

use std::path::PathBuf;

use predicates::prelude::PredicateBooleanExt;
use serde_json::json;
use support::{Fixture, fixture, write};

fn rules(fx: &Fixture) -> assert_cmd::assert::Assert {
    fx.revy_review().arg("rules").assert()
}

fn index_header(fx: &Fixture) -> String {
    format!(
        "Rule files: {}/<topic>/<id>.md\n",
        fx.rules.path().display()
    )
}

#[test]
fn shipped_rules_validate_and_are_indexed() {
    let fx = fixture();
    let shipped = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../skill/rules")
        .canonicalize()
        .unwrap();
    let mut state: serde_json::Value =
        serde_json::from_str(&support::read(fx.dir.path(), "state.json")).unwrap();
    state["rules"] = json!(shipped);
    fx.write_state(&state);

    let assert = rules(&fx);

    let stdout = String::from_utf8(assert.success().get_output().stdout.clone()).unwrap();
    assert!(stdout.contains(
        "\nedge-of-range-change (logic) [any] — Comparison (`<`/`<=`), range or slice bound"
    ));
}

#[test]
fn index_lists_the_rules_for_the_changes_languages() {
    let fx = fixture();

    let assert = rules(&fx);

    assert.success().stdout(format!(
        "{}edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n\
         arc-mutex-single-owner (rust) [rust] — The diff wraps state in `Arc<Mutex<_>>`.\n",
        index_header(&fx)
    ));
}

#[test]
fn any_folder_in_the_rules_directory_is_a_topic() {
    let fx = fixture();
    write(
        fx.rules.path(),
        "team/duplicated-logic.md",
        "# duplicated-logic\nlangs: python, Rust\n\nWhen: always\nAsk: Does equivalent logic already exist?\n",
    );

    let assert = rules(&fx);

    assert.success().stdout(predicates::str::ends_with(
        "duplicated-logic (team) [python, rust] — always\n",
    ));
}

#[test]
fn files_at_the_top_of_the_rules_directory_are_not_rules() {
    let fx = fixture();
    write(fx.rules.path(), "README.md", "# Review rules\n");

    let assert = rules(&fx);

    assert
        .success()
        .stdout(predicates::str::contains("README").not());
}

#[test]
fn rule_without_when_is_an_error_naming_its_file() {
    let fx = fixture();
    write(
        fx.rules.path(),
        "logic/no-when.md",
        "# no-when\nlangs: any\nAsk: a\n",
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{}/logic/no-when.md: rule no-when has no `When: …` line\n",
        fx.rules.path().display()
    ));
}

#[test]
fn rule_without_langs_is_an_error_naming_its_file() {
    let fx = fixture();
    write(
        fx.rules.path(),
        "logic/no-langs.md",
        "# no-langs\nWhen: always\n",
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{}/logic/no-langs.md: rule no-langs has no `langs: …` line\n",
        fx.rules.path().display()
    ));
}

#[test]
fn langs_naming_no_language_is_an_error() {
    let fx = fixture();
    write(
        fx.rules.path(),
        "logic/commas.md",
        "# commas\nlangs: , ,\nWhen: always\n",
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{}/logic/commas.md: `langs: , ,` names no language; write `any` or a list\n",
        fx.rules.path().display()
    ));
}

#[test]
fn same_id_in_two_topics_is_an_error() {
    let fx = fixture();
    write(
        fx.rules.path(),
        "rust/edge-of-range-change.md",
        support::EDGE_RULE,
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{0}/rust/edge-of-range-change.md: rule id edge-of-range-change is also {0}/logic/edge-of-range-change.md\n",
        fx.rules.path().display()
    ));
}

#[test]
fn broken_rule_is_reported_and_the_valid_rules_are_still_indexed() {
    let fx = fixture();
    write(
        fx.rules.path(),
        "logic/no-when.md",
        "# no-when\nlangs: any\n",
    );

    let assert = rules(&fx);

    assert.failure().code(1).stdout(format!(
        "{}edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n\
         arc-mutex-single-owner (rust) [rust] — The diff wraps state in `Arc<Mutex<_>>`.\n",
        index_header(&fx)
    ));
}

#[test]
fn missing_rules_directory_is_an_actionable_error() {
    let fx = fixture();
    let gone = fx.rules.path().join("gone");
    let mut state: serde_json::Value =
        serde_json::from_str(&support::read(fx.dir.path(), "state.json")).unwrap();
    state["rules"] = json!(gone);
    fx.write_state(&state);

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "rules directory {} (state.json `rules`) does not exist\n",
        gone.display()
    ));
}

// --- the plug-in folder, ~/.revy-review/ -----------------------------------------

const TEAM_RULE: &str = "# no-print-debugging\nlangs: python\n\nWhen: The diff adds a `print(` call.\nAsk: Should it be a log call?\n";

#[test]
fn plugin_rules_are_indexed_after_the_skills_under_their_own_header() {
    let fx = fixture();
    write(&fx.plugin(), "rules/team/no-print-debugging.md", TEAM_RULE);

    let assert = rules(&fx);

    assert.success().stdout(format!(
        "{}edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n\
         arc-mutex-single-owner (rust) [rust] — The diff wraps state in `Arc<Mutex<_>>`.\n\
         Rule files: {}/rules/<topic>/<id>.md\n\
         no-print-debugging (team) [python] — The diff adds a `print(` call.\n",
        index_header(&fx),
        fx.plugin().display()
    ));
}

#[test]
fn plugin_rule_reusing_a_built_in_id_is_an_error_naming_both_files() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "rules/team/edge-of-range-change.md",
        support::EDGE_RULE,
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{}/rules/team/edge-of-range-change.md: rule id edge-of-range-change is also {}/logic/edge-of-range-change.md\n",
        fx.plugin().display(),
        fx.rules.path().display()
    ));
}

#[test]
fn empty_plugin_folder_leaves_the_built_in_index() {
    let fx = fixture();
    std::fs::create_dir_all(fx.plugin()).unwrap();

    let assert = rules(&fx);

    assert.success().stdout(format!(
        "{}edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n\
         arc-mutex-single-owner (rust) [rust] — The diff wraps state in `Arc<Mutex<_>>`.\n",
        index_header(&fx)
    ));
}

#[test]
fn plugin_reviewers_are_listed_numbered_after_the_four_built_ins() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "Authorization on every changed handler.\n",
    );
    write(
        &fx.plugin(),
        "reviewers/api-compat.md",
        "Wire compatibility with older clients.\n",
    );

    let assert = rules(&fx);

    assert.success().stdout(predicates::str::ends_with(format!(
        "Reviewer 5: api-compat — {0}/reviewers/api-compat.md\n\
         Reviewer 6: security — {0}/reviewers/security.md\n",
        fx.plugin().display()
    )));
}

#[test]
fn blank_plugin_reviewer_is_an_error_naming_its_file_and_is_not_listed() {
    let fx = fixture();
    write(&fx.plugin(), "reviewers/security.md", " \n\n");

    let assert = rules(&fx);

    assert
        .failure()
        .code(1)
        .stdout(predicates::str::contains("Reviewer").not())
        .stderr(format!(
            "{}/reviewers/security.md: plug-in reviewer security is blank; write its prompt: what it reviews and goes deepest on\n",
            fx.plugin().display()
        ));
}
