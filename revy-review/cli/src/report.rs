//! `report.md`: rendered from `verified.json`, edited by the owner, and read
//! back only for what posting needs; every other line is the owner's.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::diff::Change;
use crate::investigations::Unresolved;
use crate::model::{Depth, Diagnostic, Severity, Sha, Site, Sites, Text, VerifiedId};
use crate::proof::{self, TestSource};
use crate::verified::Verified;

pub const FILE: &str = "report.md";
const OWN_SECTION: &str = "## My findings";

/// `depth` (absent when the gate stopped the review before one was chosen) and `skipped`,
/// `(name, reason)` of each specialist the coordinator did not dispatch, are listed so the
/// owner sees a wrong call.
///
/// A proven finding's body carries its reproduction and, from `sources`, its test's code, so
/// the comment posted from it does too.
pub fn render(
    target: &Text,
    head: &Sha,
    verified: &Verified,
    sources: &BTreeMap<VerifiedId, TestSource>,
    unresolved: &[Unresolved],
    depth: Option<&Depth>,
    skipped: &[(&str, &Text)],
) -> String {
    let mut out = format!("# Review: {target}\n\n");
    for finding in &verified.findings {
        let sites: Vec<String> = finding.sites.all().map(ToString::to_string).collect();
        let _ = write!(
            out,
            "### {}. {}\n- post: yes\n- severity: {}\n- where: {}\n\n{}\n\n{}Fix: {}\n\n",
            finding.id,
            finding.title,
            finding.severity.as_str(),
            sites.join(", "),
            finding.breaks.as_str().trim(),
            finding
                .proof
                .as_ref()
                .map(|proof| proof::render(proof, sources.get(&finding.id)) + "\n\n")
                .unwrap_or_default(),
            finding.fix.as_str().trim(),
        );
    }
    if verified
        .findings
        .iter()
        .any(|finding| finding.proof.is_some())
    {
        let branch = proof::merged_branch(head);
        let _ = write!(
            out,
            "## Proofs\n\nThe reproducing tests are on local branch `{branch}`, never pushed: check it out to run them; `git branch -D {branch}` drops it.\n\n"
        );
    }
    if !unresolved.is_empty() {
        out.push_str("## Suspicions (unconfirmed)\n\n");
    }
    for item in unresolved {
        let _ = write!(
            out,
            "### {}\n- where: {}\n- evidence: {}\n- open question: {}\n- check: {}\n\n",
            item.claim.as_str().trim(),
            item.site,
            item.evidence.as_str().trim(),
            item.open_question.as_str().trim(),
            item.check.as_str().trim(),
        );
    }
    if let Some(depth) = depth {
        let _ = write!(
            out,
            "## Review depth\n\n- {}: {}\n\n",
            depth.level.as_str(),
            depth.why.as_str().trim()
        );
    }
    if !skipped.is_empty() {
        out.push_str("## Specialists skipped\n\n");
        for (name, reason) in skipped {
            let _ = writeln!(out, "- {name}: {}", reason.as_str().trim());
        }
        out.push('\n');
    }
    out.push_str(OWN_SECTION);
    out.push('\n');
    out
}

/// A `post: yes` finding block as the owner left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posted {
    pub where_line: usize,
    pub title: String,
    pub severity: Severity,
    pub sites: Sites,
    pub breaks: String,
    pub fix: String,
}

/// `- site [severity] — text` under `## My findings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Own {
    pub line: usize,
    pub site: Site,
    pub severity: Severity,
    pub text: Text,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub posted: Vec<Posted>,
    /// Finding blocks marked `post: no`.
    pub held: usize,
    pub own: Vec<Own>,
}

type Lines<'a> = Vec<(usize, &'a str)>;

/// Finding blocks (`### F<k>. <title>`) and own findings; every other section is skipped.
pub fn parse(text: &str) -> Result<Report, Vec<Diagnostic>> {
    let mut sections: Vec<(usize, &str, Lines<'_>)> = Vec::new();
    let mut fence: Option<&str> = None;
    for (number, line) in (1..).zip(text.lines()) {
        let heading = fence.is_none() && (line.starts_with("## ") || line.starts_with("### "));
        fence = next_fence(fence, line);
        if heading {
            sections.push((number, line.trim_end(), Vec::new()));
        } else if let Some((_, _, body)) = sections.last_mut() {
            body.push((number, line));
        }
    }
    let mut report = Report::default();
    let mut errors = Vec::new();
    for (number, heading, body) in &sections {
        if let Some(title) = finding_title(heading) {
            match parse_finding(*number, title, body) {
                Ok(Some(posted)) => report.posted.push(posted),
                Ok(None) => report.held += 1,
                Err(mut found) => errors.append(&mut found),
            }
        } else if *heading == OWN_SECTION {
            for &(line, text) in body.iter().filter(|(_, text)| text.starts_with("- ")) {
                match parse_own(line, text) {
                    Ok(own) => report.own.push(own),
                    Err(error) => errors.push(error),
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(report)
    } else {
        Err(errors)
    }
}

/// The open code fence after `line`: a test's code may hold lines that look like headings.
fn next_fence<'a>(open: Option<&'a str>, line: &'a str) -> Option<&'a str> {
    let trimmed = line.trim();
    let run = trimmed
        .get(..trimmed.len() - trimmed.trim_start_matches('`').len())
        .unwrap_or_default();
    match open {
        None if run.len() >= 3 => Some(run),
        Some(open) if run.len() >= open.len() && run == trimmed => None,
        open => open,
    }
}

fn at(line: usize, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(format!("{FILE}:{line}"), message)
}

fn finding_title(heading: &str) -> Option<&str> {
    let (id, title) = heading.strip_prefix("### ")?.split_once(". ")?;
    id.parse::<VerifiedId>().ok()?;
    Some(title.trim())
}

/// `- <key>: <value>` for the keys a finding block's header holds.
fn field(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.strip_prefix("- ")?.split_once(':')?;
    ["post", "severity", "where"]
        .contains(&key)
        .then(|| (key, value.trim()))
}

/// `- post:`, `- severity:` and `- where:` lines, then why it breaks, then `Fix: …`.
fn parse_finding(
    number: usize,
    title: &str,
    body: &[(usize, &str)],
) -> Result<Option<Posted>, Vec<Diagnostic>> {
    let meta_len = body
        .iter()
        .take_while(|(_, line)| line.trim().is_empty() || field(line).is_some())
        .count();
    let (meta, text) = body.split_at(meta_len);
    let value = |key: &str| {
        meta.iter().find_map(|&(line, text)| {
            field(text).and_then(|(found, value)| (found == key).then_some((line, value)))
        })
    };
    let missing = |key: &str| vec![at(number, format!("finding block has no `- {key}:` line"))];
    match value("post").ok_or_else(|| missing("post"))? {
        (_, "yes") => {}
        (_, "no") => return Ok(None),
        (line, other) => {
            return Err(vec![at(
                line,
                format!("post must be yes or no, not `{other}`"),
            )]);
        }
    }
    let severity = value("severity")
        .ok_or_else(|| missing("severity"))
        .and_then(|(line, raw)| {
            raw.parse::<Severity>()
                .map_err(|message| vec![at(line, message)])
        });
    let sites = value("where")
        .ok_or_else(|| missing("where"))
        .and_then(|(line, raw)| {
            raw.split(',')
                .map(|site| site.trim().parse::<Site>())
                .collect::<Result<Vec<_>, _>>()
                .and_then(|sites| Sites::try_from(sites).map_err(str::to_owned))
                .map(|sites| (line, sites))
                .map_err(|message| vec![at(line, message)])
        });
    let lines: Vec<&str> = text.iter().map(|(_, line)| *line).collect();
    let body = lines
        .iter()
        .rposition(|line| line.starts_with("Fix:"))
        .and_then(|index| {
            let (breaks, fix) = lines.split_at(index);
            let breaks = breaks.join("\n").trim().to_owned();
            let fix = fix.join("\n").trim_start_matches("Fix:").trim().to_owned();
            (!breaks.is_empty() && !fix.is_empty()).then_some((breaks, fix))
        })
        .ok_or_else(|| {
            vec![at(
                number,
                "finding body needs why it breaks, then a `Fix: …` line",
            )]
        });
    match (severity, sites, body) {
        (Ok(severity), Ok((where_line, sites)), Ok((breaks, fix))) => Ok(Some(Posted {
            where_line,
            title: title.to_owned(),
            severity,
            sites,
            breaks,
            fix,
        })),
        (severity, sites, body) => Err([severity.err(), sites.err(), body.err()]
            .into_iter()
            .flatten()
            .flatten()
            .collect()),
    }
}

fn parse_own(number: usize, line: &str) -> Result<Own, Diagnostic> {
    let malformed = || {
        at(
            number,
            format!(
                "expected `- path:line [severity] — text`, found `{}`",
                line.trim_end()
            ),
        )
    };
    let rest = line.strip_prefix("- ").ok_or_else(malformed)?;
    let (site, rest) = rest.split_once(" [").ok_or_else(malformed)?;
    let (severity, text) = rest.split_once("] — ").ok_or_else(malformed)?;
    Ok(Own {
        line: number,
        site: site.trim().parse().map_err(|message| at(number, message))?,
        severity: severity.parse().map_err(|message| at(number, message))?,
        text: text.trim().parse().map_err(|_| malformed())?,
    })
}

/// Comments a forge cannot place inline, because their site is outside the diff.
pub fn outside_diff(report: &Report, change: &Change) -> Vec<Diagnostic> {
    let posted = report
        .posted
        .iter()
        .map(|posted| (posted.where_line, posted.sites.first()));
    let own = report.own.iter().map(|own| (own.line, &own.site));
    posted
        .chain(own)
        .filter(|(_, site)| !change.shows(site))
        .map(|(line, site)| {
            at(
                line,
                format!("{site} is outside the diff; it would post as a general comment"),
            )
        })
        .collect()
}

const fn label(severity: Severity) -> &'static str {
    match severity {
        Severity::Blocking => "issue (blocking)",
        Severity::NonBlocking => "issue (non-blocking)",
        Severity::Question => "question (non-blocking)",
        Severity::Suggestion => "suggestion (non-blocking)",
    }
}

/// Post-ready comments: location, conventional-comments header, why, fix.
pub fn comments(report: &Report) -> String {
    let posted = report.posted.iter().map(|posted| {
        let (first, others) = (posted.sites.first(), posted.sites.rest());
        let also = if others.is_empty() {
            String::new()
        } else {
            let others: Vec<String> = others.iter().map(ToString::to_string).collect();
            format!("Also at {}.\n\n", others.join(", "))
        };
        format!(
            "{first}\n{}: {}\n\n{}\n\n{also}Fix: {}\n",
            label(posted.severity),
            posted.title,
            posted.breaks,
            posted.fix
        )
    });
    let own = report
        .own
        .iter()
        .map(|own| format!("{}\n{}: {}\n", own.site, label(own.severity), own.text));
    posted.chain(own).collect::<Vec<_>>().join("---\n")
}
