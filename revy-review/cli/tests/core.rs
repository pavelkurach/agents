#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::collections::BTreeMap;

use revy_review::diff::{Change, HeadFiles};
use revy_review::model::{RepoPath, Site};
use revy_review::report;
use revy_review::rules::{self, Langs};
use revy_review::verified::Verified;

fn site(raw: &str) -> Site {
    raw.parse().unwrap()
}

const DIFF: &str = "diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1,3 +10,4 @@ fn f() {
@@ -40 +50 @@
diff --git a/gone.rs b/gone.rs
--- a/gone.rs
+++ /dev/null
@@ -1,2 +0,0 @@
";

#[test]
fn hunk_lines_including_context_are_shown_by_the_forge() {
    let change = Change::parse("a.rs\n", DIFF);

    let shown: Vec<bool> = ["a.rs:9", "a.rs:10", "a.rs:13", "a.rs:14", "a.rs:50"]
        .iter()
        .map(|raw| change.shows(&site(raw)))
        .collect();

    assert_eq!(shown, [false, true, true, false, true]);
}

#[test]
fn deleted_files_are_not_changed_files_at_head() {
    let change = Change::parse("a.rs\n", DIFF);

    let files: Vec<&str> = change.files().map(RepoPath::as_str).collect();

    assert_eq!(files, ["a.rs"]);
}

#[test]
fn site_without_a_line_number_is_rejected() {
    let parsed = "src/a.rs".parse::<Site>();

    assert_eq!(
        parsed,
        Err("site `src/a.rs` is not `path:line` (repo-relative path, line from 1)".to_owned())
    );
}

#[test]
fn site_at_line_zero_is_rejected() {
    let parsed = "src/a.rs:0".parse::<Site>();

    assert!(parsed.is_err());
}

#[test]
fn absolute_site_path_is_rejected() {
    let parsed = "/etc/passwd:1".parse::<Site>();

    assert!(parsed.is_err());
}

#[test]
fn site_path_escaping_the_repository_is_rejected() {
    let parsed = "src/../../etc/passwd:1".parse::<Site>();

    assert!(parsed.is_err());
}

fn five_lines() -> HeadFiles {
    HeadFiles::new(BTreeMap::from([(
        "a.rs".parse().unwrap(),
        Some("one\ntwo\nthree\nfour\nfive\n".to_owned()),
    )]))
}

#[test]
fn quote_three_lines_away_is_near_enough() {
    let found = five_lines().quote_near(&site("a.rs:1"), "four");

    assert_eq!(found, Ok(()));
}

#[test]
fn quote_four_lines_away_is_too_far() {
    let found = five_lines().quote_near(&site("a.rs:1"), "five");

    assert_eq!(
        found,
        Err("quote `five` is not within ±3 lines of a.rs:1 at head".to_owned())
    );
}

#[test]
fn quote_spanning_lines_is_rejected() {
    let found = five_lines().quote_near(&site("a.rs:1"), "one\ntwo");

    assert_eq!(
        found,
        Err("quote spans several lines; copy the one line at the first site".to_owned())
    );
}

#[test]
fn rule_reads_langs_and_when_and_leaves_the_rest_to_the_reviewer() {
    let text = "# x\nlangs: Python, rust\n\nWhen: w\nAsk: a\nExample:\n  bad:  # not a heading\n";

    let rule = rules::parse("python", "x", text).unwrap();

    assert_eq!(
        (rule.langs, rule.when.as_str()),
        (
            Langs::Only(["python".to_owned(), "rust".to_owned()].into()),
            "w"
        )
    );
}

#[test]
fn file_language_comes_from_its_extension() {
    let langs: Vec<Option<&str>> = [
        "a.rs", "b.pyi", "c.go", "d.tsx", "e.mjs", "f.md", "Makefile",
    ]
    .iter()
    .map(|path| rules::lang_of(&path.parse().unwrap()))
    .collect();

    assert_eq!(
        langs,
        [
            Some("rust"),
            Some("python"),
            Some("go"),
            Some("typescript"),
            Some("typescript"),
            None,
            None
        ]
    );
}

#[test]
fn rendered_report_parses_back_to_the_verified_findings() {
    let verified: Verified = serde_json::from_str(
        r#"{"findings": [{"id": "F1", "title": "t", "severity": "suggestion", "sites": ["a.rs:1"],
             "quote": "q", "breaks": "b\n\n- more b", "fix": "f", "rule": null, "merged_from": []}],
            "dropped": []}"#,
    )
    .unwrap();
    let rendered = report::render(
        &"feature".parse().unwrap(),
        &"abc1234".parse().unwrap(),
        &verified,
        &[],
        &[],
    );

    let parsed = report::parse(&rendered).unwrap();

    let posted = &parsed.posted[0];
    assert_eq!(
        (
            posted.title.as_str(),
            posted.breaks.as_str(),
            posted.fix.as_str()
        ),
        ("t", "b\n\n- more b", "f")
    );
}
