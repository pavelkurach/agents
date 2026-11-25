#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

mod support;

use serde_json::{Value, json};
use support::{
    Fixture, PROOF_TEST, empty_review, fixture, proof, review_with_two_findings, valid_problem,
    valid_review, valid_verified, write,
};

fn check(fx: &Fixture) -> assert_cmd::assert::Assert {
    fx.revy_review().arg("check").assert()
}

fn with_problem(problem: &Value) -> Fixture {
    let fx = fixture();
    fx.write_json("problem.json", problem);
    fx
}

fn with_review(review: &Value) -> Fixture {
    let fx = fixture();
    fx.write_json("reviews/1.json", review);
    fx
}

fn with_extra_ledger_entry(entry: Value) -> Fixture {
    let mut review = valid_review();
    review["ledger"].as_array_mut().unwrap().push(entry);
    with_review(&review)
}

// --- problem.json --------------------------------------------------------------

#[test]
fn valid_problem_passes() {
    let fx = with_problem(&valid_problem());

    let assert = check(&fx);

    assert
        .success()
        .stdout("problem.json: ok (2 criteria, blocks: true)\n");
}

#[test]
fn problem_blocks_flag_disagreeing_with_criteria_is_rejected() {
    let mut problem = valid_problem();
    problem["blocks"] = json!(false);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert.failure().code(1).stderr(
        "problem.json: blocks: is false but the criteria give true (core criterion C1 is not_met)\n",
    );
}

#[test]
fn problem_with_no_criterion_met_blocks() {
    let mut problem = valid_problem();
    problem["criteria"][0]["verdict"] = json!("partly");
    problem["criteria"][1]["verdict"] = json!("unverifiable");
    problem["blocks"] = json!(false);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert.failure().stderr(
        "problem.json: blocks: is false but the criteria give true (no criterion is met)\n",
    );
}

#[test]
fn implied_criterion_not_met_does_not_block() {
    let mut problem = valid_problem();
    problem["criteria"][0]["verdict"] = json!("met");
    problem["criteria"][1]["verdict"] = json!("not_met");
    problem["blocks"] = json!(false);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert
        .success()
        .stdout("problem.json: ok (2 criteria, blocks: false)\n");
}

#[test]
fn problem_without_criteria_is_rejected() {
    let mut problem = valid_problem();
    problem["criteria"] = json!([]);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("problem.json: criteria: no criteria; list at least one, quoted or implied\n");
}

#[test]
fn unquoted_criterion_not_marked_implied_is_rejected() {
    let mut problem = valid_problem();
    problem["criteria"][0]["quote"] = json!(null);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "problem.json: criteria[0]: C1: quote the source with implied false, or set quote null with implied true",
    ));
}

#[test]
fn duplicate_criterion_id_is_rejected() {
    let mut problem = valid_problem();
    problem["criteria"][1]["id"] = json!("C1");
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("problem.json: criteria[1]: id is already used by criteria[0]\n");
}

#[test]
fn evidence_in_a_file_missing_at_head_is_rejected() {
    let mut problem = valid_problem();
    problem["criteria"][1]["evidence"] = json!(["src/gone.rs:1"]);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("problem.json: criteria[1].evidence[0]: src/gone.rs does not exist at head\n");
}

#[test]
fn met_criterion_without_evidence_is_rejected() {
    let mut problem = valid_problem();
    problem["criteria"][1]["evidence"] = json!([]);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("problem.json: criteria[1]: verdict met needs at least one evidence site\n");
}

#[test]
fn scope_creep_entry_with_no_sites_is_rejected() {
    let mut problem = valid_problem();
    problem["scope_creep"][0]["sites"] = json!([]);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "problem.json: scope_creep[0].sites: sites must name at least one `path:line`",
    ));
}

#[test]
fn scope_creep_duplicate_id_is_rejected() {
    let mut problem = valid_problem();
    let entry = problem["scope_creep"][0].clone();
    problem["scope_creep"] = json!([entry.clone(), entry]);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("problem.json: scope_creep[1]: id is already used by scope_creep[0]\n");
}

#[test]
fn scope_creep_id_not_s_number_is_rejected() {
    let mut problem = valid_problem();
    problem["scope_creep"][0]["id"] = json!("scope:src/util.py:1");
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::contains(
        "`scope:src/util.py:1` is not a scope-creep id `S<k>`",
    ));
}

#[test]
fn scope_creep_site_past_the_end_of_the_file_is_rejected() {
    let mut problem = valid_problem();
    problem["scope_creep"][0]["sites"] = json!(["src/util.py:1", "src/util.py:9"]);
    let fx = with_problem(&problem);

    let assert = check(&fx);

    assert.failure().stderr(
        "problem.json: scope_creep[0].sites[1]: line 9 is past the end of src/util.py (2 lines)\n",
    );
}

// --- review directory ----------------------------------------------------------

#[test]
fn directory_without_state_is_an_actionable_error() {
    let fx = fixture();
    let elsewhere = tempfile::tempdir().unwrap();

    let assert = fx.revy_review().arg("check").arg(elsewhere.path()).assert();

    assert.failure().code(1).stderr(format!(
        "{} has no state.json; run from the review directory or pass it\n",
        elsewhere.path().display()
    ));
}

#[test]
fn state_without_a_skill_directory_is_rejected() {
    let fx = fixture();
    let mut state: Value =
        serde_json::from_str(&support::read(fx.dir.path(), "state.json")).unwrap();
    state.as_object_mut().unwrap().remove("skill");
    fx.write_state(&state);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "state.json: missing field `skill`",
    ));
}

#[test]
fn every_unparsable_file_is_reported() {
    let fx = fixture();
    write(fx.dir.path(), "reviews/1.json", "{");
    write(fx.dir.path(), "verified.json", "[]");

    let assert = check(&fx);

    assert
        .failure()
        .stderr(predicates::str::is_match("^verified.json: .*\nreviews/1.json: .*\n$").unwrap());
}

// --- reviews/<n>.json ----------------------------------------------------------

#[test]
fn valid_review_passes() {
    let fx = with_review(&valid_review());

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/1.json: ok (1 findings, 2 ledger entries)\n");
}

#[test]
fn quote_not_near_its_site_is_rejected() {
    let mut review = valid_review();
    review["findings"][0]["quote"] = json!("items.iter().sum()");
    let fx = with_review(&review);

    let assert = check(&fx);

    assert.failure().code(1).stderr(
        "reviews/1.json: findings[0]: quote `items.iter().sum()` is not within ±3 lines of src/calc.rs:2 at head\n",
    );
}

#[test]
fn finding_site_past_the_end_of_the_file_is_rejected() {
    let mut review = valid_review();
    review["findings"][0]["sites"] = json!(["src/calc.rs:2", "src/calc.rs:9"]);
    let fx = with_review(&review);

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/1.json: findings[0].sites[1]: line 9 is past the end of src/calc.rs (3 lines)\n",
    );
}

#[test]
fn ledger_site_past_the_end_of_the_file_is_rejected() {
    let mut review = valid_review();
    review["ledger"][0]["site"] = json!("src/calc.rs:40");
    let fx = with_review(&review);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("reviews/1.json: ledger[0]: line 40 is past the end of src/calc.rs (3 lines)\n");
}

#[test]
fn unknown_rule_id_is_rejected() {
    let mut review = valid_review();
    review["findings"][0]["rule"] = json!("no-such-rule");
    let fx = with_review(&review);

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/1.json: findings[0]: rule no-such-rule does not exist; cite an id from `revy-review rules`\n",
    );
}

#[test]
fn finding_citing_a_plug_in_rule_passes() {
    let mut review = valid_review();
    review["findings"][0]["rule"] = json!("no-print-debugging");
    let fx = with_review(&review);
    write(
        &fx.plugin(),
        "rules/team/no-print-debugging.md",
        "# no-print-debugging\nlangs: python\n\nWhen: The diff adds a `print(` call.\n",
    );

    let assert = check(&fx);

    assert.success();
}

#[test]
fn bad_severity_is_a_schema_error_naming_the_field() {
    let mut review = valid_review();
    review["findings"][0]["severity"] = json!("major");
    let fx = with_review(&review);

    let assert = check(&fx);

    assert.failure().code(1).stderr(predicates::str::starts_with(
        "reviews/1.json: findings[0].severity: unknown variant `major`, expected one of `blocking`, `non-blocking`, `question`, `suggestion`",
    ));
}

#[test]
fn finding_id_of_another_reviewer_is_rejected() {
    let mut review = valid_review();
    review["findings"][0]["id"] = json!("2.1");
    review["ledger"][0]["finding"] = json!("2.1");
    let fx = with_review(&review);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("reviews/1.json: findings[0]: id 2.1 does not belong to reviewer 1; use 1.<k>\n");
}

#[test]
fn duplicate_finding_id_is_rejected() {
    let mut review = valid_review();
    let finding = review["findings"][0].clone();
    review["findings"].as_array_mut().unwrap().push(finding);
    let fx = with_review(&review);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("reviews/1.json: findings[1]: id is already used by findings[0]\n");
}

#[test]
fn reviewer_number_must_match_the_file_name() {
    let fx = fixture();
    fx.write_json("reviews/5.json", &valid_review());

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/5.json: reviewer: reviewer 1 does not match the file name 5.json; reviewer n writes reviews/<n>.json\n",
    );
}

#[test]
fn changed_file_without_ledger_entry_is_rejected() {
    let mut review = valid_review();
    review["ledger"][1]["site"] = json!("src/calc.rs:1");
    let fx = with_review(&review);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("reviews/1.json: ledger: changed file src/util.py has no ledger entry\n");
}

#[test]
fn investigator_file_with_empty_ledger_is_accepted() {
    let fx = fixture();
    let mut review = valid_review();
    review["reviewer"] = json!(5);
    review["lens"] = json!("investigator");
    review["ledger"] = json!([]);
    review["findings"][0]["id"] = json!("5.1");
    fx.write_json("reviews/5.json", &review);

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/5.json: ok (1 findings, 0 ledger entries)\n");
}

#[test]
fn reviewer_file_with_empty_ledger_is_rejected() {
    let mut review = empty_review(1);
    review["lens"] = json!("boundaries");
    review["ledger"] = json!([]);
    let fx = with_review(&review);

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/1.json: ledger: changed file src/calc.rs has no ledger entry\nreviews/1.json: ledger: changed file src/util.py has no ledger entry\n",
    );
}

#[test]
fn specialist_review_with_an_empty_ledger_is_accepted() {
    let mut review = empty_review(5);
    review["lens"] = json!("security");
    review["ledger"] = json!([]);
    let fx = fixture();
    fx.write_json("reviews/5.json", &review);
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "Authorization on every changed handler.\n",
    );
    fx.record_specialists(&json!({"security": "dispatched"}));

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/5.json: ok (0 findings, 0 ledger entries)\n");
}

#[test]
fn review_numbered_past_the_specialists_is_rejected() {
    let mut review = empty_review(5);
    review["lens"] = json!("duplication");
    review["ledger"] = json!([]);
    let fx = fixture();
    fx.write_json("reviews/5.json", &review);

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/5.json: reviewer: there is no reviewer 5: the 4 lens reviewers and the specialists state.json records as dispatched are 1..4, and only the investigator numbers after them\n",
    );
}

#[test]
fn finding_verdict_naming_an_absent_finding_is_rejected() {
    let mut review = valid_review();
    review["ledger"][0]["finding"] = json!("1.9");
    let fx = with_review(&review);

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/1.json: ledger[0]: verdict `finding` names 1.9, which is not a finding in this file\n",
    );
}

#[test]
fn fine_verdict_naming_a_finding_is_rejected() {
    let mut review = valid_review();
    review["ledger"][1]["finding"] = json!("1.1");
    let fx = with_review(&review);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[1]: finding 1.1 is only valid with verdict `finding`; set it to null",
    ));
}

#[test]
fn finding_verdict_without_a_finding_id_is_rejected() {
    let mut review = valid_review();
    review["ledger"][0]["finding"] = json!(null);
    let fx = with_review(&review);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[0]: verdict `finding` needs the finding id it produced",
    ));
}

#[test]
fn ledger_entry_with_an_unknown_field_is_rejected() {
    let mut entry = contract_fine_entry();
    entry["walked"] = json!("total(&[1, 2]) returns 3 at head");
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert
        .failure()
        .stderr(predicates::str::contains("unknown field `walked`"));
}

fn contract_fine_entry() -> Value {
    json!({"kind": "contract", "subject": "total sums all items", "site": "src/util.py:1",
           "verdict": "fine", "finding": null})
}

// --- unit edge-case matrix -----------------------------------------------------

fn unit_entry(cases: &Value) -> Value {
    json!({"kind": "unit", "subject": "total", "site": "src/util.py:1",
           "verdict": "fine", "finding": null, "cases": cases,
           "probe": {"cmd": "python3 probe.py", "output": "total([]) = 0"}})
}

fn full_cases() -> Value {
    json!({"size": "n/a: scalar", "boundaries": "empty slice returns 0", "absence": "n/a: no options",
           "shape": "n/a: flat", "variants": "n/a: one type", "state": "n/a: pure",
           "failure": "overflow wraps in release", "concurrency": "n/a: no sharing",
           "vs_base": "base summed with sum(items); head's loop returns the same total"})
}

#[test]
fn unit_with_full_floor_is_accepted() {
    let fx = with_extra_ledger_entry(unit_entry(&full_cases()));

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/1.json: ok (1 findings, 3 ledger entries)\n");
}

#[test]
fn unit_with_floor_and_extra_dimension_is_accepted() {
    let mut cases = full_cases();
    cases["encoding"] = json!("utf-8 only");
    let fx = with_extra_ledger_entry(unit_entry(&cases));

    let assert = check(&fx);

    assert.success();
}

#[test]
fn unit_missing_two_floor_dimensions_is_rejected_listing_both() {
    let mut cases = full_cases();
    cases.as_object_mut().unwrap().remove("size");
    cases.as_object_mut().unwrap().remove("failure");
    let fx = with_extra_ledger_entry(unit_entry(&cases));

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2].cases: cases missing floor dimensions: size, failure \u{2014} answer each with the case walked and its outcome, or n/a and why",
    ));
}

#[test]
fn unit_without_vs_base_is_rejected() {
    let mut cases = full_cases();
    cases.as_object_mut().unwrap().remove("vs_base");
    let fx = with_extra_ledger_entry(unit_entry(&cases));

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2].cases: cases missing floor dimensions: vs_base \u{2014} answer each with the case walked and its outcome, or n/a and why",
    ));
}

#[test]
fn unit_with_vs_base_comparison_is_accepted() {
    let mut cases = full_cases();
    cases["vs_base"] =
        json!("base held the lock across the fetch; head holds it too (src/util.py:4)");
    let fx = with_extra_ledger_entry(unit_entry(&cases));

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/1.json: ok (1 findings, 3 ledger entries)\n");
}

#[test]
fn new_unit_with_vs_base_new_is_accepted() {
    let mut cases = full_cases();
    cases["vs_base"] = json!("new");
    let fx = with_extra_ledger_entry(unit_entry(&cases));

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/1.json: ok (1 findings, 3 ledger entries)\n");
}

#[test]
fn unit_with_blank_floor_value_is_rejected() {
    let mut cases = full_cases();
    cases["state"] = json!("  ");
    let fx = with_extra_ledger_entry(unit_entry(&cases));

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2].cases.state: text must not be blank",
    ));
}

#[test]
fn unit_without_cases_is_rejected() {
    let mut entry = unit_entry(&full_cases());
    entry.as_object_mut().unwrap().remove("cases");
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2]: a unit entry needs cases",
    ));
}

#[test]
fn cases_on_a_contract_entry_are_rejected() {
    let mut entry = contract_fine_entry();
    entry["cases"] = full_cases();
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2]: cases is only valid on a unit entry",
    ));
}

// --- unit probe ----------------------------------------------------------------

#[test]
fn unit_with_probe_that_ran_is_accepted() {
    let mut entry = unit_entry(&full_cases());
    entry["probe"] = json!({"cmd": "python3 deep.py", "output": "depth 100000: RecursionError"});
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/1.json: ok (1 findings, 3 ledger entries)\n");
}

#[test]
fn unit_fine_with_probe_not_run_is_rejected() {
    let mut entry = unit_entry(&full_cases());
    entry["probe"] = json!({"why_not_run": "needs a CUDA toolchain"});
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2]: verdict `fine` needs a probe that ran; with probe.why_not_run the verdict is `suspect`",
    ));
}

#[test]
fn unit_suspect_with_probe_not_run_is_accepted() {
    let mut entry = unit_entry(&full_cases());
    entry["probe"] = json!({"why_not_run": "needs a CUDA toolchain"});
    entry["verdict"] = json!("suspect");
    entry["suspicion"] = suspicion("1.s1");
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/1.json: ok (1 findings, 3 ledger entries)\n");
}

#[test]
fn unit_without_probe_is_rejected() {
    let mut entry = unit_entry(&full_cases());
    entry.as_object_mut().unwrap().remove("probe");
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2]: a unit entry needs probe: {\"cmd\", \"output\"} of what ran, or {\"why_not_run\"}",
    ));
}

#[test]
fn unit_probe_with_empty_cmd_is_rejected() {
    let mut entry = unit_entry(&full_cases());
    entry["probe"] = json!({"cmd": "", "output": "total([]) = 0"});
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2].probe.cmd: text must not be blank",
    ));
}

#[test]
fn unit_probe_with_empty_output_is_rejected() {
    let mut entry = unit_entry(&full_cases());
    entry["probe"] = json!({"cmd": "python3 probe.py", "output": ""});
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2].probe.output: text must not be blank",
    ));
}

#[test]
fn unit_probe_with_cmd_but_no_output_is_rejected() {
    let mut entry = unit_entry(&full_cases());
    entry["probe"] = json!({"cmd": "python3 probe.py"});
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2].probe: probe is either {\"cmd\", \"output\"} (what ran and what it printed) or {\"why_not_run\"}",
    ));
}

#[test]
fn probe_on_a_contract_entry_is_rejected() {
    let mut entry = contract_fine_entry();
    entry["probe"] = json!({"why_not_run": "n/a"});
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2]: probe is only valid on a unit entry",
    ));
}

// --- proofs --------------------------------------------------------------------

/// Reviewer 1's finding 1.1 carrying `proof`.
fn with_review_proof(fx: &Fixture, proof: &Value) {
    let mut review = valid_review();
    review["findings"][0]["proof"] = proof.clone();
    fx.write_json("reviews/1.json", &review);
}

#[test]
fn finding_with_a_proof_committed_on_a_proofs_branch_passes() {
    let fx = fixture();
    let commit = fx.commit_on_branch("revy/x/proofs-1", "tests/calc_test.rs", PROOF_TEST);
    with_review_proof(&fx, &proof(&commit));

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/1.json: ok (1 findings, 2 ledger entries)\n");
}

#[test]
fn proof_citing_a_commit_missing_from_the_repository_is_rejected() {
    let fx = fixture();
    with_review_proof(&fx, &proof("deadbeef"));

    let assert = check(&fx);

    assert.failure().code(1).stderr(
        "reviews/1.json: findings[0].proof: commit deadbeef is not in the repository; commit the test on your proofs branch and cite that sha\n",
    );
}

#[test]
fn proof_whose_test_file_is_absent_at_its_commit_is_rejected() {
    let fx = fixture();
    let commit = fx.commit_on_branch("revy/x/proofs-1", "tests/other_test.rs", PROOF_TEST);
    with_review_proof(&fx, &proof(&commit));

    let assert = check(&fx);

    assert.failure().stderr(format!(
        "reviews/1.json: findings[0].proof: test file tests/calc_test.rs does not exist at commit {commit}\n"
    ));
}

#[test]
fn proof_with_a_blank_failing_output_is_rejected() {
    let fx = fixture();
    let mut blank = proof(&fx.head);
    blank["head"] = json!("  ");
    with_review_proof(&fx, &blank);

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/1.json: findings[0].proof.head: text must not be blank at line 36 column 20\n",
    );
}

#[test]
fn proof_test_without_a_test_name_is_rejected() {
    let fx = fixture();
    let mut unnamed = proof(&fx.head);
    unnamed["test"] = json!("tests/calc_test.rs");
    with_review_proof(&fx, &unnamed);

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/1.json: findings[0].proof.test: test `tests/calc_test.rs` is not `<repo-relative path>::<test name or line>` at line 33 column 36\n",
    );
}

#[test]
fn kept_finding_whose_proof_is_off_the_merged_proofs_branch_is_rejected() {
    let fx = with_two_findings();
    let commit = fx.commit_on_branch("revy/x/proofs-1", "tests/calc_test.rs", PROOF_TEST);
    let mut verified = valid_verified();
    verified["findings"][0]["proof"] = proof(&commit);
    fx.write_json("verified.json", &verified);

    let assert = check(&fx);

    assert.failure().stderr(format!(
        "verified.json: findings[0].proof: commit {commit} is not on branch {}; cherry-pick each kept finding's proving commit there and cite the new sha\n",
        fx.proofs_branch()
    ));
}

#[test]
fn reviewer_proof_whose_branch_was_deleted_after_verification_passes() {
    let fx = with_verified(&valid_verified());
    let mut review = review_with_two_findings();
    review["findings"][0]["proof"] = proof("deadbeef");
    fx.write_json("reviews/1.json", &review);

    let assert = check(&fx);

    assert.success();
}

// --- suspicions ----------------------------------------------------------------

fn suspicion(id: &str) -> Value {
    json!({"id": id, "claim": "first() may be called with an empty list",
           "evidence": "no caller in the diff guards the list",
           "open_question": "does any caller outside the diff pass an empty list",
           "tried": "grepped the diff for callers of first"})
}

fn suspect_entry(suspicion: &Value) -> Value {
    json!({"kind": "contract", "subject": "first never sees an empty list", "site": "src/util.py:2",
           "verdict": "suspect", "finding": null, "suspicion": suspicion})
}

#[test]
fn suspect_entry_passes_until_the_verifier_runs_without_investigations() {
    let fx = with_extra_ledger_entry(suspect_entry(&suspicion("1.s1")));

    let assert = check(&fx);

    assert
        .success()
        .stdout("reviews/1.json: ok (1 findings, 3 ledger entries)\n");
}

#[test]
fn suspect_entry_without_open_question_is_rejected() {
    let mut incomplete = suspicion("1.s1");
    incomplete.as_object_mut().unwrap().remove("open_question");
    let fx = with_extra_ledger_entry(suspect_entry(&incomplete));

    let assert = check(&fx);

    assert
        .failure()
        .stderr(predicates::str::contains("missing field `open_question`"));
}

#[test]
fn suspect_entry_with_blank_tried_is_rejected() {
    let mut blank = suspicion("1.s1");
    blank["tried"] = json!("  ");
    let fx = with_extra_ledger_entry(suspect_entry(&blank));

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2].suspicion.tried: text must not be blank",
    ));
}

#[test]
fn suspect_entry_without_a_suspicion_is_rejected() {
    let mut entry = suspect_entry(&suspicion("1.s1"));
    entry.as_object_mut().unwrap().remove("suspicion");
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2]: verdict `suspect` needs the suspicion it raised",
    ));
}

#[test]
fn suspicion_on_a_fine_entry_is_rejected() {
    let mut entry = contract_fine_entry();
    entry["suspicion"] = suspicion("1.s1");
    let fx = with_extra_ledger_entry(entry);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "reviews/1.json: ledger[2]: suspicion is only valid with verdict `suspect`",
    ));
}

#[test]
fn duplicate_suspicion_id_in_one_file_is_rejected() {
    let mut review = valid_review();
    let ledger = review["ledger"].as_array_mut().unwrap();
    ledger.push(suspect_entry(&suspicion("1.s1")));
    ledger.push(suspect_entry(&suspicion("1.s1")));
    let fx = with_review(&review);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("reviews/1.json: ledger[3]: suspicion id 1.s1 is already used by ledger[2]\n");
}

#[test]
fn suspicion_id_of_another_reviewer_is_rejected() {
    let fx = with_extra_ledger_entry(suspect_entry(&suspicion("2.s1")));

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/1.json: ledger[2]: suspicion id 2.s1 does not belong to reviewer 1; use 1.s<k>\n",
    );
}

// --- investigations.json -------------------------------------------------------

fn with_one_suspicion() -> Fixture {
    let mut review = valid_review();
    review["ledger"]
        .as_array_mut()
        .unwrap()
        .push(suspect_entry(&suspicion("1.s1")));
    let fx = with_review(&review);
    fx.write_empty_reviews_2_to_4();
    fx
}

fn investigation(outcome: &str, finding: &Value, check: &Value) -> Value {
    json!({"items": [{"suspicion": "1.s1", "outcome": outcome,
                      "reason": "traced every caller", "evidence": "src/util.py:2 is only called with xs of length 2",
                      "finding": finding, "check": check}]})
}

fn investigator_review() -> Value {
    let mut review = valid_review();
    review["reviewer"] = json!(5);
    review["lens"] = json!("investigator");
    review["ledger"] = json!([]);
    review["findings"][0]["id"] = json!("5.1");
    review
}

#[test]
fn investigations_resolving_every_suspicion_pass() {
    let fx = with_one_suspicion();
    fx.write_json(
        "investigations.json",
        &investigation("refuted", &json!(null), &json!(null)),
    );

    let assert = check(&fx);

    assert.success().stdout(
        "reviews/1.json: ok (1 findings, 3 ledger entries)\nreviews/2.json: ok (0 findings, 2 ledger entries)\nreviews/3.json: ok (0 findings, 2 ledger entries)\nreviews/4.json: ok (0 findings, 2 ledger entries)\ninvestigations.json: ok (1 items)\n",
    );
}

#[test]
fn investigations_missing_a_suspicion_are_rejected() {
    let fx = with_one_suspicion();
    fx.write_json("investigations.json", &json!({"items": []}));

    let assert = check(&fx);

    assert.failure().stderr(
        "investigations.json: suspicion 1.s1 has no item; the investigator must resolve every suspicion exactly once\n",
    );
}

#[test]
fn investigation_of_an_unknown_suspicion_is_rejected() {
    let fx = with_one_suspicion();
    let mut doc = investigation("refuted", &json!(null), &json!(null));
    doc["items"][0]["suspicion"] = json!("1.s7");
    fx.write_json("investigations.json", &doc);

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "investigations.json: items[0]: 1.s7 is not a suspicion in any reviews/*.json\n",
    ));
}

#[test]
fn suspicion_resolved_twice_is_rejected() {
    let fx = with_one_suspicion();
    let mut doc = investigation("refuted", &json!(null), &json!(null));
    let item = doc["items"][0].clone();
    doc["items"].as_array_mut().unwrap().push(item);
    fx.write_json("investigations.json", &doc);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("investigations.json: items[1]: suspicion is already resolved by items[0]\n");
}

#[test]
fn confirmed_naming_an_investigator_finding_passes() {
    let fx = with_one_suspicion();
    fx.write_json("reviews/5.json", &investigator_review());
    fx.write_json(
        "investigations.json",
        &investigation("confirmed", &json!("5.1"), &json!(null)),
    );

    let assert = check(&fx);

    assert.success();
}

#[test]
fn confirmed_naming_a_missing_finding_is_rejected() {
    let fx = with_one_suspicion();
    fx.write_json(
        "investigations.json",
        &investigation("confirmed", &json!("5.1"), &json!(null)),
    );

    let assert = check(&fx);

    assert.failure().stderr(
        "investigations.json: items[0]: confirmed names finding 5.1, which is not a finding in any reviews/*.json; append it to reviews/<m>.json first\n",
    );
}

#[test]
fn confirmed_naming_a_reviewers_finding_is_rejected() {
    let fx = with_one_suspicion();
    fx.write_json(
        "investigations.json",
        &investigation("confirmed", &json!("1.1"), &json!(null)),
    );

    let assert = check(&fx);

    assert.failure().stderr(
        "investigations.json: items[0]: confirmed names finding 1.1, which belongs to a reviewer; confirmed must name a finding in the review file with lens `investigator`\n",
    );
}

#[test]
fn confirmed_without_a_finding_id_is_rejected() {
    let fx = with_one_suspicion();
    fx.write_json(
        "investigations.json",
        &investigation("confirmed", &json!(null), &json!(null)),
    );

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::contains(
        "1.s1: outcome `confirmed` needs the finding id appended to the investigator's review file",
    ));
}

#[test]
fn unresolved_with_a_check_passes() {
    let fx = with_one_suspicion();
    fx.write_json(
        "investigations.json",
        &investigation(
            "unresolved",
            &json!(null),
            &json!("call first([]) from the billing job"),
        ),
    );

    let assert = check(&fx);

    assert.success();
}

#[test]
fn unresolved_without_a_check_is_rejected() {
    let fx = with_one_suspicion();
    fx.write_json(
        "investigations.json",
        &investigation("unresolved", &json!(null), &json!(null)),
    );

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::contains(
        "1.s1: outcome `unresolved` needs a check a human can run quickly",
    ));
}

#[test]
fn refuted_with_a_check_is_rejected() {
    let fx = with_one_suspicion();
    fx.write_json(
        "investigations.json",
        &investigation("refuted", &json!(null), &json!("run it")),
    );

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::contains(
        "1.s1: finding is only valid with `confirmed` and check only with `unresolved`; set the other to null",
    ));
}

// --- verified.json -------------------------------------------------------------

/// Reviews 1 and 2, with findings 1.1 and 1.2 that `valid_verified` keeps.
fn with_two_findings() -> Fixture {
    let fx = with_review(&review_with_two_findings());
    fx.write_empty_reviews_2_to_4();
    fx
}

fn with_verified(verified: &Value) -> Fixture {
    let fx = with_two_findings();
    fx.write_json("verified.json", verified);
    fx
}

#[test]
fn verified_accounting_for_every_finding_passes() {
    let fx = with_verified(&valid_verified());

    let assert = check(&fx);

    assert.success().stdout(
        "reviews/1.json: ok (2 findings, 2 ledger entries)\nreviews/2.json: ok (0 findings, 2 ledger entries)\nreviews/3.json: ok (0 findings, 2 ledger entries)\nreviews/4.json: ok (0 findings, 2 ledger entries)\nverified.json: ok (2 kept, 0 dropped)\n",
    );
}

#[test]
fn reviewer_finding_neither_merged_nor_dropped_is_rejected() {
    let mut verified = valid_verified();
    verified["findings"].as_array_mut().unwrap().pop();
    let fx = with_verified(&verified);

    let assert = check(&fx);

    assert.failure().code(1).stderr(
        "verified.json: reviewer finding 1.2 (reviews/1.json) is neither merged into a kept finding nor dropped\n",
    );
}

#[test]
fn reviewer_finding_both_merged_and_dropped_is_rejected() {
    let mut verified = valid_verified();
    verified["dropped"] = json!([{"from": "1.2", "refuted_by": "src/util.py:1", "why": "guarded"}]);
    let fx = with_verified(&verified);

    let assert = check(&fx);

    assert.failure().stderr(
        "verified.json: reviewer finding 1.2 (reviews/1.json) appears 2 times (findings[1].merged_from[0], dropped[0].from); it must appear exactly once\n",
    );
}

#[test]
fn merged_id_not_in_any_review_is_rejected() {
    let mut verified = valid_verified();
    verified["findings"][0]["merged_from"] = json!(["1.1", "3.1"]);
    let fx = with_verified(&verified);

    let assert = check(&fx);

    assert.failure().stderr(
        "verified.json: findings[0].merged_from[1]: 3.1 is not a finding in any reviews/*.json\n",
    );
}

#[test]
fn refuting_site_past_the_end_of_the_file_is_rejected() {
    let mut verified = valid_verified();
    verified["findings"].as_array_mut().unwrap().pop();
    verified["dropped"] = json!([{"from": "1.2", "refuted_by": "src/util.py:9", "why": "guarded"}]);
    let fx = with_verified(&verified);

    let assert = check(&fx);

    assert.failure().stderr(
        "verified.json: dropped[0].refuted_by: line 9 is past the end of src/util.py (2 lines)\n",
    );
}

#[test]
fn verified_quote_not_near_its_site_is_rejected() {
    let mut verified = valid_verified();
    verified["findings"][1]["quote"] = json!("def first(xs):");
    verified["findings"][1]["sites"] = json!(["src/calc.rs:3"]);
    let fx = with_verified(&verified);

    let assert = check(&fx);

    assert.failure().stderr(
        "verified.json: findings[1]: quote `def first(xs):` is not within ±3 lines of src/calc.rs:3 at head\n",
    );
}

#[test]
fn verified_unknown_rule_id_is_rejected() {
    let mut verified = valid_verified();
    verified["findings"][0]["rule"] = json!("logic/edge-of-range-change");
    let fx = with_verified(&verified);

    let assert = check(&fx);

    assert.failure().stderr(
        "verified.json: findings[0]: rule logic/edge-of-range-change does not exist; cite an id from `revy-review rules`\n",
    );
}

#[test]
fn duplicate_verified_id_is_rejected() {
    let mut verified = valid_verified();
    verified["findings"][1]["id"] = json!("F1");
    let fx = with_verified(&verified);

    let assert = check(&fx);

    assert
        .failure()
        .stderr("verified.json: findings[1]: id is already used by findings[0]\n");
}

#[test]
fn verified_reads_sites_at_the_head_commit_not_the_worktree() {
    let fx = with_verified(&valid_verified());
    write(fx.repo.path(), "src/calc.rs", "changed after head\n");

    let assert = check(&fx);

    assert.success();
}

fn with_gate_and_verified(verified: &Value) -> Fixture {
    let fx = with_verified(verified);
    fx.write_json("problem.json", &valid_problem());
    fx
}

#[test]
fn gate_findings_merged_by_id_pass() {
    let mut verified = valid_verified();
    verified["findings"][0]["merged_from"] = json!(["1.1", "C1"]);
    verified["findings"][1]["merged_from"] = json!(["1.2", "S1"]);
    let fx = with_gate_and_verified(&verified);

    let assert = check(&fx);

    assert.success();
}

#[test]
fn gate_finding_neither_merged_nor_dropped_is_rejected() {
    let mut verified = valid_verified();
    verified["findings"][0]["merged_from"] = json!(["1.1", "C1"]);
    let fx = with_gate_and_verified(&verified);

    let assert = check(&fx);

    assert.failure().stderr(
        "verified.json: gate finding S1 (scope creep) is neither merged into a kept finding nor dropped\n",
    );
}

#[test]
fn criterion_that_is_met_is_not_a_gate_finding() {
    let mut verified = valid_verified();
    verified["findings"][0]["merged_from"] = json!(["1.1", "C1", "C2"]);
    verified["findings"][1]["merged_from"] = json!(["1.2", "S1"]);
    let fx = with_gate_and_verified(&verified);

    let assert = check(&fx);

    assert.failure().stderr(
        "verified.json: findings[0].merged_from[2]: C2 is not a not_met or partly criterion in problem.json\n",
    );
}

#[test]
fn scope_creep_id_unknown_to_problem_is_rejected() {
    let mut verified = valid_verified();
    verified["findings"][0]["merged_from"] = json!(["1.1", "C1"]);
    verified["findings"][1]["merged_from"] = json!(["1.2", "S1", "S2"]);
    let fx = with_gate_and_verified(&verified);

    let assert = check(&fx);

    assert.failure().stderr(
        "verified.json: findings[1].merged_from[2]: S2 is not a scope_creep entry in problem.json\n",
    );
}

// --- verified.json: every reviewer and suspicion accounted for ------------------

const NEEDS_1_TO_4: &str = "the 4 lens reviewers and the specialists state.json records as dispatched need reviews/1.json..4.json, one subagent each\n";

fn with_reviews_and_empty_verified(numbers: &[u64]) -> Fixture {
    let fx = fixture();
    for n in numbers {
        fx.write_json(&format!("reviews/{n}.json"), &empty_review(*n));
    }
    fx.write_json("verified.json", &json!({"findings": [], "dropped": []}));
    fx
}

#[test]
fn verified_with_a_reviewer_file_missing_is_rejected() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3]);

    let assert = check(&fx);

    assert
        .failure()
        .stderr(format!("reviews/4.json missing: {NEEDS_1_TO_4}"));
}

fn gate_dropped(why: &str) -> Value {
    json!({"findings": [], "dropped": [
        {"from": "C1", "refuted_by": "src/calc.rs:2", "why": why},
        {"from": "S1", "refuted_by": "src/util.py:1", "why": why}
    ]})
}

#[test]
fn verified_with_a_specialist_and_its_file_passes() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3, 4, 5]);
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "Authorization on every changed handler.\n",
    );
    fx.record_specialists(&json!({"security": "dispatched"}));

    let assert = check(&fx);

    assert.success();
}

#[test]
fn verified_with_a_dispatched_specialist_but_no_file_for_it_is_rejected() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3, 4]);
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "Authorization on every changed handler.\n",
    );
    fx.record_specialists(&json!({"security": "dispatched"}));

    let assert = check(&fx);

    assert.failure().stderr(
        "reviews/5.json missing: the 4 lens reviewers and the specialists state.json records as dispatched need reviews/1.json..5.json, one subagent each\n",
    );
}

const MIGRATIONS_WHEN_RELEVANT: &str =
    "dispatch: when relevant\n\nSchema migrations on live tables.\n";

#[test]
fn verified_with_a_skipped_when_relevant_specialist_needs_no_file_for_it() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3, 4]);
    write(
        &fx.plugin(),
        "reviewers/migrations.md",
        MIGRATIONS_WHEN_RELEVANT,
    );
    fx.record_specialists(&json!({"migrations": {"skipped": "the change adds no migration"}}));

    let assert = check(&fx);

    assert.success();
}

#[test]
fn skipping_an_always_specialist_is_rejected() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3, 4]);
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "Authorization on every changed handler.\n",
    );
    fx.record_specialists(&json!({"security": {"skipped": "no handler changed"}}));

    let assert = check(&fx);

    assert.failure().stderr(format!(
        "state.json: specialists.security: skipped, but {}/reviewers/security.md has `dispatch: always`; dispatch it\n",
        fx.plugin().display()
    ));
}

#[test]
fn dispatched_specialists_number_on_from_5_past_a_skipped_one() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3, 4, 5]);
    write(
        &fx.plugin(),
        "reviewers/migrations.md",
        MIGRATIONS_WHEN_RELEVANT,
    );
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "Authorization on every changed handler.\n",
    );
    fx.record_specialists(&json!({
        "migrations": {"skipped": "the change adds no migration"},
        "security": "dispatched"
    }));

    let assert = check(&fx);

    assert.success();
}

#[test]
fn specialist_without_a_recorded_decision_is_rejected() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3, 4, 5]);
    write(
        &fx.plugin(),
        "reviewers/migrations.md",
        MIGRATIONS_WHEN_RELEVANT,
    );
    write(
        &fx.plugin(),
        "reviewers/security.md",
        "Authorization on every changed handler.\n",
    );
    fx.record_specialists(&json!({"security": "dispatched"}));

    let assert = check(&fx);

    assert.failure().stderr(
        "state.json: specialists: no decision for specialist migrations; record it as dispatched or skipped\n",
    );
}

#[test]
fn reviews_with_specialists_but_no_recorded_decisions_are_rejected() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3, 4]);
    write(
        &fx.plugin(),
        "reviewers/migrations.md",
        MIGRATIONS_WHEN_RELEVANT,
    );

    let assert = check(&fx);

    assert.failure().stderr(
        "state.json: specialists: missing; record a decision for every specialist `revy-review rules` lists before dispatching reviewers: `\"<name>\": \"dispatched\"` or `\"<name>\": {\"skipped\": \"<reason>\"}`\n",
    );
}

#[test]
fn decision_for_an_unknown_specialist_is_rejected() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3, 4]);
    fx.record_specialists(&json!({"security": "dispatched"}));

    let assert = check(&fx);

    assert.failure().stderr(
        "state.json: specialists.security: no such specialist; `revy-review rules` lists them\n",
    );
}

#[test]
fn skip_without_a_reason_is_rejected() {
    let fx = fixture();
    fx.record_specialists(&json!({"migrations": {"skipped": " "}}));

    let assert = check(&fx);

    assert.failure().stderr(predicates::str::starts_with(
        "state.json: specialists.migrations.skipped: text must not be blank",
    ));
}

#[test]
fn verified_with_a_blocking_gate_and_no_reviews_is_accepted() {
    let fx = with_problem(&valid_problem());
    fx.write_json("verified.json", &gate_dropped("stopped at the gate"));

    let assert = check(&fx);

    assert.success().stdout(
        "problem.json: ok (2 criteria, blocks: true)\nverified.json: ok (0 kept, 2 dropped)\n",
    );
}

#[test]
fn verified_with_a_blocking_gate_and_every_reviewer_present_is_accepted() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3, 4]);
    fx.write_json("problem.json", &valid_problem());
    fx.write_json("verified.json", &gate_dropped("deep review ran anyway"));

    let assert = check(&fx);

    assert.success();
}

#[test]
fn verified_with_a_blocking_gate_and_a_partial_review_set_is_rejected() {
    let fx = with_reviews_and_empty_verified(&[1, 2, 3]);
    fx.write_json("problem.json", &valid_problem());
    fx.write_json("verified.json", &gate_dropped("partial"));

    let assert = check(&fx);

    assert
        .failure()
        .stderr(format!("reviews/4.json missing: {NEEDS_1_TO_4}"));
}

#[test]
fn verified_with_suspicions_but_no_investigations_is_rejected() {
    let fx = with_one_suspicion();
    fx.write_json(
        "verified.json",
        &json!({"findings": [],
        "dropped": [{"from": "1.1", "refuted_by": "src/calc.rs:2", "why": "x"}]}),
    );

    let assert = check(&fx);

    assert.failure().stderr(
        "investigations.json: missing; reviews hold suspicions (1.s1) that the investigator must resolve\n",
    );
}

#[test]
fn verified_with_resolved_suspicions_passes() {
    let fx = with_one_suspicion();
    fx.write_json(
        "investigations.json",
        &investigation("refuted", &json!(null), &json!(null)),
    );
    fx.write_json(
        "verified.json",
        &json!({"findings": [],
        "dropped": [{"from": "1.1", "refuted_by": "src/calc.rs:2", "why": "x"}]}),
    );

    let assert = check(&fx);

    assert.success();
}

#[test]
fn verified_must_account_for_a_confirmed_investigator_finding() {
    let fx = with_one_suspicion();
    fx.write_json("reviews/5.json", &investigator_review());
    fx.write_json(
        "investigations.json",
        &investigation("confirmed", &json!("5.1"), &json!(null)),
    );
    fx.write_json(
        "verified.json",
        &json!({"findings": [],
        "dropped": [{"from": "1.1", "refuted_by": "src/calc.rs:2", "why": "x"}]}),
    );

    let assert = check(&fx);

    assert.failure().stderr(
        "verified.json: reviewer finding 5.1 (reviews/5.json) is neither merged into a kept finding nor dropped\n",
    );
}
