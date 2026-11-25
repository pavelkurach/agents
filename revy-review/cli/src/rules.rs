//! Rule files `<topic>/<id>.md` (format: `skill/rules/README.md`):
//! the optional `langs:` hint and the `When:` line, and the index reviewers read.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::model::{Diagnostic, repeats};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub topic: String,
    /// The file name without `.md`; findings cite it.
    pub id: String,
    /// Free-text hint for reviewers (`python`, `java, kotlin`); never interpreted.
    pub langs: Option<String>,
    pub when: String,
}

/// The rest of a rule file is for the reviewer; only `langs:` and `When:` are read; they make the index.
pub fn parse(topic: &str, id: &str, text: &str) -> Result<Rule, String> {
    let line = |label: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(label))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("rule {id} has no `{label} …` line"))
    };
    let langs = line("langs:").ok().map(str::to_owned);
    Ok(Rule {
        topic: topic.to_owned(),
        id: id.to_owned(),
        langs,
        when: line("When:")?.to_owned(),
    })
}

/// The rules of one rules directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    pub dir: PathBuf,
    pub rules: Vec<Rule>,
}

impl RuleSet {
    fn file(&self, rule: &Rule) -> String {
        format!("{}/{}/{}.md", self.dir.display(), rule.topic, rule.id)
    }
}

/// Rules sharing an id, within or across rules directories, which findings could not tell apart.
pub fn duplicates(sets: &[RuleSet]) -> Vec<Diagnostic> {
    let rules: Vec<(String, &Rule)> = sets
        .iter()
        .flat_map(|set| set.rules.iter().map(move |rule| (set.file(rule), rule)))
        .collect();
    repeats(&rules, |(_, rule)| rule.id.clone())
        .into_iter()
        .filter_map(|(index, first)| Some((rules.get(index)?, rules.get(first)?)))
        .map(|((file, rule), (first, _))| {
            Diagnostic::new(file, format!("rule id {} is also {first}", rule.id))
        })
        .collect()
}

/// Per rules directory, a `Rule files:` header and one line per rule of the given topics
/// (`None`: every topic): `<id> (<topic>) [<langs>] — <When>`, without `[<langs>]` when the rule has none.
pub fn index(sets: &[RuleSet], topics: Option<&BTreeSet<String>>) -> String {
    let mut out = String::new();
    for set in sets {
        let _ = writeln!(out, "Rule files: {}/<topic>/<id>.md", set.dir.display());
        for rule in set
            .rules
            .iter()
            .filter(|rule| topics.is_none_or(|topics| topics.contains(&rule.topic)))
        {
            let hint = rule
                .langs
                .as_ref()
                .map_or_else(String::new, |langs| format!(" [{langs}]"));
            let _ = writeln!(out, "{} ({}){hint} — {}", rule.id, rule.topic, rule.when);
        }
    }
    out
}
