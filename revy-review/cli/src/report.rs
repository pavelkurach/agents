//! `report.md`: rendered from `verified.json`, edited by the owner, and read
//! back only for what posting needs; every other line is the owner's.

use std::fmt::Write as _;

use crate::diff::Change;
use crate::investigations::Unresolved;
use crate::model::{Diagnostic, Severity, Site, Sites, Text, VerifiedId};
use crate::verified::Verified;

pub const FILE: &str = "report.md";
const OWN_SECTION: &str = "## My findings";

/// `skipped`: `(name, reason)` of each specialist the coordinator did not dispatch, listed so
/// the owner sees a wrong skip.
pub fn render(
    target: &Text,
    verified: &Verified,
    unresolved: &[Unresolved],
    skipped: &[(&str, &Text)],
) -> String {
    let mut out = format!("# Review: {target}\n\n");
    for finding in &verified.findings {
        let sites: Vec<String> = finding.sites.all().map(ToString::to_string).collect();
        let _ = write!(
            out,
            "### {}. {}\n- post: yes\n- severity: {}\n- where: {}\n\n{}\n\nFix: {}\n\n",
            finding.id,
            finding.title,
            finding.severity.as_str(),
            sites.join(", "),
            finding.breaks.as_str().trim(),
            finding.fix.as_str().trim(),
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
    for (number, line) in (1..).zip(text.lines()) {
        if line.starts_with("## ") || line.starts_with("### ") {
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
