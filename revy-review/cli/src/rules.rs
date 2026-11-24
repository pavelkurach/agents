//! Rule files `<topic>/<id>.md` (format: `skill/rules/README.md`):
//! the `langs:` and `When:` lines, and the index reviewers read.

use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::model::{Diagnostic, RepoPath, repeats};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Langs {
    Any,
    Only(BTreeSet<String>),
}

impl Langs {
    fn parse(raw: &str) -> Option<Self> {
        if raw == "any" {
            return Some(Self::Any);
        }
        let langs: BTreeSet<String> = raw
            .split(',')
            .map(|lang| lang.trim().to_lowercase())
            .filter(|lang| !lang.is_empty())
            .collect();
        (!langs.is_empty()).then_some(Self::Only(langs))
    }

    fn covers_any(&self, wanted: &BTreeSet<&str>) -> bool {
        match self {
            Self::Any => true,
            Self::Only(langs) => langs.iter().any(|lang| wanted.contains(lang.as_str())),
        }
    }
}

impl fmt::Display for Langs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => f.write_str("any"),
            Self::Only(langs) => {
                f.write_str(&Vec::from_iter(langs.iter().map(String::as_str)).join(", "))
            }
        }
    }
}

/// The `langs:` name of a file's language, from its extension.
pub fn lang_of(path: &RepoPath) -> Option<&'static str> {
    match path.as_str().rsplit_once('.')?.1 {
        "rs" => Some("rust"),
        "py" | "pyi" => Some("python"),
        "go" => Some("go"),
        "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" => Some("typescript"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub topic: String,
    /// The file name without `.md`; findings cite it.
    pub id: String,
    pub langs: Langs,
    pub when: String,
}

/// The rest of a rule file is for the reviewer; only `langs:` and `When:` drive the index.
pub fn parse(topic: &str, id: &str, text: &str) -> Result<Rule, String> {
    let line = |label: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(label))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("rule {id} has no `{label} …` line"))
    };
    let langs = line("langs:")?;
    Ok(Rule {
        topic: topic.to_owned(),
        id: id.to_owned(),
        langs: Langs::parse(langs)
            .ok_or_else(|| format!("`langs: {langs}` names no language; write `any` or a list"))?,
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

/// Per rules directory, a `Rule files:` header and one line per rule for the change's
/// languages, of the given topics (`None`: every topic): `<id> (<topic>) [<langs>] — <When>`.
pub fn index(
    sets: &[RuleSet],
    langs: &BTreeSet<&str>,
    topics: Option<&BTreeSet<String>>,
) -> String {
    let mut out = String::new();
    for set in sets {
        let _ = writeln!(out, "Rule files: {}/<topic>/<id>.md", set.dir.display());
        for rule in set.rules.iter().filter(|rule| {
            rule.langs.covers_any(langs) && topics.is_none_or(|topics| topics.contains(&rule.topic))
        }) {
            let _ = writeln!(
                out,
                "{} ({}) [{}] — {}",
                rule.id, rule.topic, rule.langs, rule.when
            );
        }
    }
    out
}
