#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use assert_cmd::Command;

fn revy_plan() -> Command {
    Command::cargo_bin("revy-plan").expect("binary `revy-plan` builds")
}

fn top_help() -> String {
    let assert = revy_plan().arg("--help").assert().success();
    String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 help")
}

#[test]
fn help_lists_every_public_command() {
    let stdout = top_help();
    for command in [
        "init", "schema", "format", "status", "apply", "check", "gate", "render", "serve",
    ] {
        assert!(
            stdout.contains(command),
            "`{command}` missing from:\n{stdout}"
        );
    }
}

#[test]
fn help_omits_the_removed_run_command() {
    let stdout = top_help();
    assert!(
        !stdout.contains(" run "),
        "`run` should be gone from:\n{stdout}"
    );
}

#[test]
fn an_unknown_subcommand_exits_with_a_usage_error() {
    revy_plan().arg("nonsense").assert().code(2);
}

#[test]
fn the_version_flag_prints_the_package_version() {
    revy_plan()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn revise_help_states_the_wave_a_revision_enters() {
    let assert = revy_plan().args(["revise", "--help"]).assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 help");

    assert!(
        stdout.contains("the wave after the last recorded review round"),
        "{stdout}"
    );
}
