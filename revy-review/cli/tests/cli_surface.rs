#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn revy_review() -> Command {
    Command::cargo_bin("revy-review").unwrap()
}

#[test]
fn help_lists_every_command_and_succeeds() {
    let assert = revy_review().arg("--help").assert();

    assert.success().stdout(
        predicates::str::starts_with("Usage: revy-review <command> [dir]\n")
            .and(predicates::str::contains("\n  rules "))
            .and(predicates::str::contains("\n  check "))
            .and(predicates::str::contains("\n  sync "))
            .and(predicates::str::contains("\n  comments ")),
    );
}

#[test]
fn version_prints_the_package_version() {
    let assert = revy_review().arg("--version").assert();

    assert
        .success()
        .stdout(format!("revy-review {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn no_command_is_a_usage_error() {
    let assert = revy_review().assert();

    assert.code(2).stderr(predicates::str::starts_with(
        "error: missing command\n\nUsage:",
    ));
}

#[test]
fn unknown_command_is_a_usage_error() {
    let assert = revy_review().arg("start").assert();

    assert.code(2).stderr(predicates::str::starts_with(
        "error: unknown command `start`\n\nUsage:",
    ));
}

#[test]
fn a_second_directory_is_a_usage_error() {
    let assert = revy_review().args(["check", "a", "b"]).assert();

    assert.code(2).stderr(predicates::str::starts_with(
        "error: unexpected argument `b`\n\nUsage:",
    ));
}
