#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

mod support;

use predicates::prelude::PredicateBooleanExt;
use support::{
    Fixture, PROOF_TEST, fixture, proof, read, review_with_two_findings, valid_verified, write,
};

const RENDERED: &str = "# Review: feature

### F1. total drops the last item
- post: yes
- severity: blocking
- where: src/calc.rs:2

total(&[1, 2]) returns 1, so every invoice is short by its last line.

Fix: Sum every item: items.iter().sum().

### F2. first panics on an empty list
- post: yes
- severity: question
- where: src/util.py:2, src/calc.rs:1

first([]) raises IndexError.

Fix: Return None for an empty list.

## My findings
";

fn with_verified() -> Fixture {
    let fx = fixture();
    fx.write_json("verified.json", &valid_verified());
    fx.write_json("reviews/1.json", &review_with_two_findings());
    fx.write_empty_reviews_2_to_4();
    fx
}

fn with_report(edited: &str) -> Fixture {
    let fx = with_verified();
    write(fx.dir.path(), "report.md", edited);
    fx
}

fn sync(fx: &Fixture) -> assert_cmd::assert::Assert {
    fx.revy_review().arg("sync").assert()
}

fn comments(fx: &Fixture) -> assert_cmd::assert::Assert {
    fx.revy_review().arg("comments").assert()
}

// --- sync ----------------------------------------------------------------------

#[test]
fn sync_without_report_renders_it_from_verified() {
    let fx = with_verified();

    let assert = sync(&fx);

    assert
        .success()
        .stdout("wrote report.md: 2 findings (2 to post), 0 own findings\n");
    assert_eq!(read(fx.dir.path(), "report.md"), RENDERED);
}

#[test]
fn sync_takes_the_review_directory_as_an_argument() {
    let fx = with_verified();
    let elsewhere = tempfile::tempdir().unwrap();

    let assert = fx
        .revy_review()
        .current_dir(elsewhere.path())
        .arg("sync")
        .arg(fx.dir.path())
        .assert();

    assert.success();
    assert_eq!(read(fx.dir.path(), "report.md"), RENDERED);
}

#[test]
fn sync_never_overwrites_the_owners_edits() {
    let edited = RENDERED.replace(
        "- post: yes\n- severity: question",
        "- post: no\n- severity: question",
    );
    let fx = with_report(&edited);

    let assert = sync(&fx);

    assert
        .success()
        .stdout("report.md: 2 findings (1 to post), 0 own findings\n");
    assert_eq!(read(fx.dir.path(), "report.md"), edited);
}

#[test]
fn sync_refuses_a_review_directory_that_does_not_check_clean() {
    let fx = with_verified();
    std::fs::remove_file(fx.dir.path().join("reviews/2.json")).unwrap();

    let assert = sync(&fx);

    assert.failure().stderr(
        "reviews/2.json missing: the 4 lens reviewers and the specialists state.json records as dispatched need reviews/1.json..4.json, one subagent each\n",
    );
    assert!(!fx.dir.path().join("report.md").exists());
}

#[test]
fn sync_without_verified_asks_for_the_verifier() {
    let fx = fixture();

    let assert = sync(&fx);

    assert
        .failure()
        .code(1)
        .stderr("verified.json does not exist; sync renders the verifier's findings\n");
}

#[test]
fn sync_rejects_a_bad_severity_at_its_line() {
    let fx = with_report(&RENDERED.replace("- severity: blocking", "- severity: major"));

    let assert = sync(&fx);

    assert.failure().stderr(
        "report.md:5: severity `major` is not one of blocking, non-blocking, question, suggestion\n",
    );
}

#[test]
fn sync_rejects_a_post_value_other_than_yes_or_no() {
    let fx = with_report(&RENDERED.replacen("- post: yes", "- post: maybe", 1));

    let assert = sync(&fx);

    assert
        .failure()
        .stderr("report.md:4: post must be yes or no, not `maybe`\n");
}

#[test]
fn sync_rejects_a_posted_block_without_where() {
    let fx = with_report(&RENDERED.replace("- where: src/calc.rs:2\n", ""));

    let assert = sync(&fx);

    assert
        .failure()
        .stderr("report.md:3: finding block has no `- where:` line\n");
}

#[test]
fn sync_rejects_a_posted_body_without_fix() {
    let fx = with_report(&RENDERED.replace(
        "Fix: Sum every item: items.iter().sum().",
        "Sum every item.",
    ));

    let assert = sync(&fx);

    assert
        .failure()
        .stderr("report.md:3: finding body needs why it breaks, then a `Fix: …` line\n");
}

#[test]
fn sync_skips_the_rest_of_a_block_not_posted() {
    let fx = with_report(&RENDERED.replacen(
        "- post: yes\n- severity: blocking",
        "- post: no\n- severity: major",
        1,
    ));

    let assert = sync(&fx);

    assert
        .success()
        .stdout("report.md: 2 findings (1 to post), 0 own findings\n");
}

#[test]
fn sync_rejects_a_malformed_own_finding() {
    let fx = with_report(&format!("{RENDERED}- src/util.py:1 — no severity\n"));

    let assert = sync(&fx);

    assert.failure().stderr(
        "report.md:22: expected `- path:line [severity] — text`, found `- src/util.py:1 — no severity`\n",
    );
}

#[test]
fn sync_warns_about_a_site_outside_the_diff() {
    let fx = with_report(&format!(
        "{RENDERED}- README.md:1 [question] — is the readme current?\n"
    ));

    let assert = sync(&fx);

    assert
        .success()
        .stdout("report.md: 2 findings (2 to post), 1 own findings\n")
        .stderr("report.md:22: warning: README.md:1 is outside the diff; it would post as a general comment\n");
}

#[test]
fn sync_ignores_sections_it_does_not_post() {
    let fx = with_report(&format!(
        "{RENDERED}\n## Notes\nChecked with the owner.\n### not a block\n- not an own finding\n\n## Extras\nfree text\n"
    ));

    let assert = sync(&fx);

    assert
        .success()
        .stdout("report.md: 2 findings (2 to post), 0 own findings\n")
        .stderr("");
}

#[test]
fn sync_lists_each_skipped_specialist_with_its_reason() {
    let fx = with_verified();
    write(
        &fx.plugin(),
        "reviewers/migrations.md",
        "dispatch: when relevant\n\nSchema migrations on live tables.\n",
    );
    fx.record_specialists(
        &serde_json::json!({"migrations": {"skipped": "the change adds no migration"}}),
    );

    let assert = sync(&fx);

    assert.success();
    assert_eq!(
        read(fx.dir.path(), "report.md"),
        RENDERED.replace(
            "## My findings\n",
            "## Specialists skipped\n\n- migrations: the change adds no migration\n\n## My findings\n"
        )
    );
}

// --- proofs --------------------------------------------------------------------

/// The total finding proven by a test on the merged proofs branch; returns the branch.
fn with_proven_finding() -> (Fixture, String) {
    let fx = with_verified();
    let branch = fx.proofs_branch();
    let commit = fx.commit_on_branch(&branch, "tests/calc_test.rs", PROOF_TEST);
    let mut verified = valid_verified();
    verified["findings"][0]["proof"] = proof(&commit);
    fx.write_json("verified.json", &verified);
    (fx, branch)
}

#[test]
fn sync_renders_a_proven_findings_reproduction_and_the_proofs_branch() {
    let (fx, branch) = with_proven_finding();

    let assert = sync(&fx);

    assert.success();
    assert_eq!(
        read(fx.dir.path(), "report.md"),
        RENDERED
            .replace(
                "short by its last line.\n\nFix:",
                "short by its last line.\n\nReproduced by `tests/calc_test.rs::total_counts_the_last_item` — `cargo test --test calc_test`:\n\n    assertion `left == right` failed\n      left: 1\n     right: 3\n\nAt base:\n\n    test total_counts_the_last_item ... ok\n\nFix:",
            )
            .replace(
                "## My findings\n",
                &format!("## Proofs\n\nThe reproducing tests are on local branch `{branch}`, never pushed: check it out to run them; `git branch -D {branch}` drops it.\n\n## My findings\n"),
            )
    );
}

#[test]
fn sync_omits_the_base_result_of_a_proof_that_cannot_run_at_base() {
    let (fx, _) = with_proven_finding();
    let mut verified: serde_json::Value =
        serde_json::from_str(&read(fx.dir.path(), "verified.json")).unwrap();
    verified["findings"][0]["proof"]["base"] = serde_json::json!("n/a");
    fx.write_json("verified.json", &verified);

    let assert = sync(&fx);

    assert.success();
    assert!(
        read(fx.dir.path(), "report.md")
            .contains("     right: 3\n\nFix: Sum every item: items.iter().sum().")
    );
}

#[test]
fn comments_carry_a_proven_findings_reproduction() {
    let (fx, _) = with_proven_finding();
    sync(&fx).success();

    let assert = comments(&fx);

    assert.success().stdout(predicates::str::starts_with(
        "src/calc.rs:2
issue (blocking): total drops the last item

total(&[1, 2]) returns 1, so every invoice is short by its last line.

Reproduced by `tests/calc_test.rs::total_counts_the_last_item` — `cargo test --test calc_test`:

    assertion `left == right` failed
      left: 1
     right: 3

At base:

    test total_counts_the_last_item ... ok

Fix: Sum every item: items.iter().sum().
---
",
    ));
}

#[test]
fn comments_never_print_the_proofs_branch() {
    let (fx, branch) = with_proven_finding();
    sync(&fx).success();

    let assert = comments(&fx);

    assert
        .success()
        .stdout(predicates::str::contains(branch).not());
}

// --- suspicions ----------------------------------------------------------------

fn with_unresolved_suspicion() -> Fixture {
    let fx = with_verified();
    let mut review = review_with_two_findings();
    review["ledger"].as_array_mut().unwrap().push(serde_json::json!(
        {"kind": "contract", "subject": "first never sees an empty list", "site": "src/util.py:2",
         "verdict": "suspect", "finding": null,
         "suspicion": {"id": "1.s1", "claim": "first() may be called with an empty list",
                       "evidence": "no caller in the diff guards the list",
                       "open_question": "does any caller outside the diff pass an empty list",
                       "tried": "grepped the diff for callers of first"}}
    ));
    fx.write_json("reviews/1.json", &review);
    fx.write_json(
        "investigations.json",
        &serde_json::json!({"items": [{"suspicion": "1.s1", "outcome": "unresolved",
            "reason": "callers live outside the repo", "evidence": "only in-repo caller passes two items",
            "finding": null, "check": "call first([]) from the billing job"}]}),
    );
    fx
}

#[test]
fn sync_renders_an_unresolved_suspicion_after_the_findings() {
    let fx = with_unresolved_suspicion();

    let assert = sync(&fx);

    assert.success();
    assert_eq!(
        read(fx.dir.path(), "report.md"),
        RENDERED.replace(
            "## My findings\n",
            "## Suspicions (unconfirmed)\n\n### first() may be called with an empty list\n- where: src/util.py:2\n- evidence: only in-repo caller passes two items\n- open question: does any caller outside the diff pass an empty list\n- check: call first([]) from the billing job\n\n## My findings\n"
        )
    );
}

#[test]
fn comments_omit_suspicions() {
    let fx = with_unresolved_suspicion();
    sync(&fx).success();

    let assert = comments(&fx);

    assert.success().stdout(
        predicates::str::contains("first() may be called")
            .not()
            .and(predicates::str::contains("billing job").not()),
    );
}

// --- comments ------------------------------------------------------------------

#[test]
fn comments_follow_the_owners_edits_to_post_severity_and_body() {
    let edited = RENDERED
        .replace("- severity: blocking", "- severity: question")
        .replace(
            "total(&[1, 2]) returns 1, so every invoice is short by its last line.",
            "An invoice of [1, 2] totals 1.",
        )
        .replace(
            "- post: yes\n- severity: question\n- where: src/util.py:2",
            "- post: no\n- severity: question\n- where: src/util.py:2",
        )
        + "- src/util.py:1 [suggestion] — `first` could say in its name that it may fail\n";
    let fx = with_report(&edited);

    let assert = comments(&fx);

    assert.success().stdout(
        "src/calc.rs:2
question (non-blocking): total drops the last item

An invoice of [1, 2] totals 1.

Fix: Sum every item: items.iter().sum().
---
src/util.py:1
suggestion (non-blocking): `first` could say in its name that it may fail
",
    );
}

#[test]
fn comments_place_a_multi_site_finding_at_its_first_site() {
    let fx = with_report(&RENDERED.replacen("- post: yes", "- post: no", 1));

    let assert = comments(&fx);

    assert.success().stdout(
        "src/util.py:2
question (non-blocking): first panics on an empty list

first([]) raises IndexError.

Also at src/calc.rs:1.

Fix: Return None for an empty list.
",
    );
}

#[test]
fn comments_label_blocking_findings_as_blocking_issues() {
    let fx = with_report(&RENDERED.replace(
        "- post: yes\n- severity: question",
        "- post: no\n- severity: question",
    ));

    let assert = comments(&fx);

    assert.success().stdout(predicates::str::starts_with(
        "src/calc.rs:2\nissue (blocking): total drops the last item\n",
    ));
}

#[test]
fn comments_need_only_report_md() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "report.md",
        &RENDERED.replacen("- post: yes", "- post: no", 2),
    );

    let assert = fixture()
        .revy_review()
        .arg("comments")
        .arg(dir.path())
        .assert();

    assert.success().stdout("");
}

#[test]
fn comments_never_print_the_notes_section() {
    let fx = with_report(&format!("{RENDERED}\n## Notes\nprivate remark\n"));

    let assert = comments(&fx);

    assert
        .success()
        .stdout(predicates::str::contains("private remark").not());
}

#[test]
fn comments_without_a_report_ask_for_sync_first() {
    let fx = with_verified();

    let assert = comments(&fx);

    assert
        .failure()
        .code(1)
        .stderr("./report.md does not exist; run `revy-review sync` first\n");
}

#[test]
fn comments_refuse_a_report_with_errors() {
    let fx = with_report(&RENDERED.replace("- severity: blocking", "- severity: major"));

    let assert = comments(&fx);

    assert.failure().code(1).stdout("");
}
