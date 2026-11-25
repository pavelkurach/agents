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
    format!("Rule files: {}/<topic>/<id>.md\n", fx.rules().display())
}

#[test]
fn shipped_rules_validate_and_are_indexed() {
    let fx = fixture();
    let shipped = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../skill")
        .canonicalize()
        .unwrap();
    let mut state: serde_json::Value =
        serde_json::from_str(&support::read(fx.dir.path(), "state.json")).unwrap();
    state["skill"] = json!(shipped);
    fx.write_state(&state);

    let assert = rules(&fx);

    let stdout = String::from_utf8(assert.success().get_output().stdout.clone()).unwrap();
    assert!(stdout.contains(
        "\nedge-of-range-change (logic) [any] — Comparison (`<`/`<=`), range or slice bound"
    ));
}

#[test]
fn index_lists_every_rule_even_when_its_langs_are_absent_from_the_change() {
    let fx = fixture();

    let assert = rules(&fx);

    assert.success().stdout(format!(
        "{}defer-in-loop (go) [go] — The diff adds a `defer` inside a loop.\n\
         edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n\
         arc-mutex-single-owner (rust) [rust] — The diff wraps state in `Arc<Mutex<_>>`.\n",
        index_header(&fx)
    ));
}

#[test]
fn any_folder_in_the_rules_directory_is_a_topic() {
    let fx = fixture();
    write(
        &fx.rules(),
        "team/duplicated-logic.md",
        "# duplicated-logic\nlangs: python, Rust\n\nWhen: always\nAsk: Does equivalent logic already exist?\n",
    );

    let assert = rules(&fx);

    assert.success().stdout(predicates::str::ends_with(
        "duplicated-logic (team) [python, Rust] — always\n",
    ));
}

#[test]
fn files_at_the_top_of_the_rules_directory_are_not_rules() {
    let fx = fixture();
    write(&fx.rules(), "README.md", "# Review rules\n");

    let assert = rules(&fx);

    assert
        .success()
        .stdout(predicates::str::contains("README").not());
}

#[test]
fn rule_without_when_is_an_error_naming_its_file() {
    let fx = fixture();
    write(
        &fx.rules(),
        "logic/no-when.md",
        "# no-when\nlangs: any\nAsk: a\n",
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{}/logic/no-when.md: rule no-when has no `When: …` line\n",
        fx.rules().display()
    ));
}

#[test]
fn rule_without_langs_line_is_indexed_without_a_hint() {
    let fx = fixture();
    write(
        &fx.rules(),
        "logic/no-langs.md",
        "# no-langs\nWhen: always\n",
    );

    let assert = rules(&fx);

    assert
        .success()
        .stdout(predicates::str::contains("\nno-langs (logic) — always\n"));
}

#[test]
fn unusual_langs_value_is_accepted_verbatim_as_a_hint() {
    let fx = fixture();
    write(
        &fx.rules(),
        "logic/odd-langs.md",
        "# odd-langs\nlangs: Java, Kotlin (JVM only)\nWhen: always\n",
    );

    let assert = rules(&fx);

    assert.success().stdout(predicates::str::contains(
        "\nodd-langs (logic) [Java, Kotlin (JVM only)] — always\n",
    ));
}

#[test]
fn same_id_in_two_topics_is_an_error() {
    let fx = fixture();
    write(
        &fx.rules(),
        "rust/edge-of-range-change.md",
        support::EDGE_RULE,
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{0}/rust/edge-of-range-change.md: rule id edge-of-range-change is also {0}/logic/edge-of-range-change.md\n",
        fx.rules().display()
    ));
}

#[test]
fn broken_rule_is_reported_and_the_valid_rules_are_still_indexed() {
    let fx = fixture();
    write(&fx.rules(), "logic/no-when.md", "# no-when\nlangs: any\n");

    let assert = rules(&fx);

    assert.failure().code(1).stdout(format!(
        "{}defer-in-loop (go) [go] — The diff adds a `defer` inside a loop.\n\
         edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n\
         arc-mutex-single-owner (rust) [rust] — The diff wraps state in `Arc<Mutex<_>>`.\n",
        index_header(&fx)
    ));
}

#[test]
fn skill_without_a_rules_directory_is_an_actionable_error() {
    let fx = fixture();
    std::fs::remove_dir_all(fx.rules()).unwrap();

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "skill directory {} (state.json `skill`) has no rules/ folder\n",
        fx.skill.path().display()
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
        "{}defer-in-loop (go) [go] — The diff adds a `defer` inside a loop.\n\
         edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n\
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
        fx.rules().display()
    ));
}

#[test]
fn empty_plugin_folder_leaves_the_built_in_index() {
    let fx = fixture();
    std::fs::create_dir_all(fx.plugin()).unwrap();

    let assert = rules(&fx);

    assert.success().stdout(format!(
        "{}defer-in-loop (go) [go] — The diff adds a `defer` inside a loop.\n\
         edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n\
         arc-mutex-single-owner (rust) [rust] — The diff wraps state in `Arc<Mutex<_>>`.\n",
        index_header(&fx)
    ));
}

#[test]
fn specialists_are_listed_skills_first_then_in_file_name_order() {
    let fx = fixture();
    write(
        fx.skill.path(),
        "reviewers/migrations.md",
        "Schema migrations on live tables.\n",
    );
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
        "Specialist: migrations (dispatch: always) — {}/reviewers/migrations.md\n\
         Specialist: api-compat (dispatch: always) — {1}/reviewers/api-compat.md\n\
         Specialist: security (dispatch: always) — {1}/reviewers/security.md\n",
        fx.skill.path().display(),
        fx.plugin().display()
    )));
}

#[test]
fn specialist_named_in_both_sources_is_an_error_naming_both_files() {
    let fx = fixture();
    write(
        fx.skill.path(),
        "reviewers/security.md",
        "Built-in focus.\n",
    );
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "The company's focus.\n",
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{}/reviewers/security.md: specialist security is also {}/reviewers/security.md\n",
        fx.plugin().display(),
        fx.skill.path().display()
    ));
}

#[test]
fn specialist_with_rules_line_gets_an_index_of_those_folders_only() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "reviewers/bounds.md",
        "rules: logic\n\nEvery changed range bound.\n",
    );

    let assert = rules(&fx);

    assert.success().stdout(predicates::str::ends_with(format!(
        "Specialist: bounds (dispatch: always) — {}/reviewers/bounds.md\n  {}  edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n",
        fx.plugin().display(),
        index_header(&fx)
    )));
}

#[test]
fn specialist_without_rules_line_is_listed_with_no_rules() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "Authorization on every changed handler.\n",
    );

    let assert = rules(&fx);

    assert.success().stdout(format!(
        "{}defer-in-loop (go) [go] — The diff adds a `defer` inside a loop.\n\
         edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n\
         arc-mutex-single-owner (rust) [rust] — The diff wraps state in `Arc<Mutex<_>>`.\n\
         Specialist: security (dispatch: always) — {}/reviewers/security.md\n",
        index_header(&fx),
        fx.plugin().display()
    ));
}

#[test]
fn rules_line_naming_an_unknown_folder_is_an_error() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "reviewers/bounds.md",
        "rules: logic, nosuch\n\nEvery changed range bound.\n",
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{}/reviewers/bounds.md: specialist bounds names rule folder `nosuch`, which no rules directory has\n",
        fx.plugin().display()
    ));
}

#[test]
fn specialist_without_a_prompt_is_an_error_naming_its_file_and_is_not_listed() {
    let fx = fixture();
    write(&fx.plugin(), "reviewers/security.md", "rules: logic\n\n");

    let assert = rules(&fx);

    assert
        .failure()
        .code(1)
        .stdout(predicates::str::contains("Specialist").not())
        .stderr(format!(
            "{}/reviewers/security.md: specialist security has no prompt; write what it reviews\n",
            fx.plugin().display()
        ));
}

#[test]
fn specialist_without_dispatch_line_is_dispatched_always() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "rules: logic\n\nAuthorization on every changed handler.\n",
    );

    let assert = rules(&fx);

    assert.success().stdout(predicates::str::contains(format!(
        "Specialist: security (dispatch: always) — {}/reviewers/security.md\n",
        fx.plugin().display()
    )));
}

#[test]
fn dispatch_when_relevant_after_the_rules_line_is_listed_with_its_mode() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "reviewers/bounds.md",
        "rules: logic\ndispatch: when relevant\n\nEvery changed range bound.\n",
    );

    let assert = rules(&fx);

    assert.success().stdout(predicates::str::ends_with(format!(
        "Specialist: bounds (dispatch: when relevant) — {}/reviewers/bounds.md\n  {}  edge-of-range-change (logic) [any] — The diff changes a comparison or range bound.\n",
        fx.plugin().display(),
        index_header(&fx)
    )));
}

#[test]
fn dispatch_line_before_the_rules_line_is_accepted() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "reviewers/bounds.md",
        "dispatch: always\nrules: logic\n\nEvery changed range bound.\n",
    );

    let assert = rules(&fx);

    assert.success().stdout(predicates::str::contains(format!(
        "Specialist: bounds (dispatch: always) — {}/reviewers/bounds.md\n  {}",
        fx.plugin().display(),
        index_header(&fx)
    )));
}

#[test]
fn dispatch_value_other_than_always_or_when_relevant_is_an_error_naming_its_file() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "dispatch: sometimes\n\nAuthorization on every changed handler.\n",
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{}/reviewers/security.md: specialist security: `dispatch: sometimes` is not `always` or `when relevant`\n",
        fx.plugin().display()
    ));
}

#[test]
fn readme_in_the_reviewers_folder_is_not_a_specialist() {
    let fx = fixture();
    write(
        fx.skill.path(),
        "reviewers/README.md",
        "# Specialists\n\nEvery file here is a specialist.\n",
    );

    let assert = rules(&fx);

    assert
        .success()
        .stdout(predicates::str::contains("Specialist").not());
}

#[test]
fn dispatch_line_given_twice_is_an_error_naming_its_file() {
    let fx = fixture();
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "dispatch: always\ndispatch: when relevant\n\nAuthorization on every changed handler.\n",
    );

    let assert = rules(&fx);

    assert.failure().code(1).stderr(format!(
        "{}/reviewers/security.md: specialist security: `dispatch:` appears twice; keep one\n",
        fx.plugin().display()
    ));
}
