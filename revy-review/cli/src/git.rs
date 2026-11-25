//! The imperative edge: git subprocesses and file reads.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::diff::{Change, HeadFiles};
use crate::model::{RepoPath, Sha};
use crate::proof::{self, Found, Proof, ProofObjects, Query, TestSource};

#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("cannot read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write {}: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("`git {args}` failed in {}: {stderr}", repo.display())]
    Git {
        repo: PathBuf,
        args: String,
        stderr: String,
    },
    #[error("{0}")]
    Usage(String),
}

/// stdout on success, stderr on failure.
fn git(repo: &Path, args: &[&str]) -> Result<Result<String, String>, ShellError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.quotePath=false"])
        .args(args)
        .output()
        .map_err(|source| ShellError::Read {
            path: PathBuf::from("git"),
            source,
        })?;
    Ok(if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    })
}

fn git_ok(repo: &Path, args: &[&str]) -> Result<String, ShellError> {
    git(repo, args)?.map_err(|stderr| ShellError::Git {
        repo: repo.to_path_buf(),
        args: args.join(" "),
        stderr,
    })
}

pub fn change(repo: &Path, base: &Sha, head: &Sha) -> Result<Change, ShellError> {
    let (base, head) = (base.as_str(), head.as_str());
    let names = git_ok(
        repo,
        &[
            "diff",
            "--name-only",
            "--diff-filter=d",
            "--no-ext-diff",
            base,
            head,
        ],
    )?;
    let unified = git_ok(
        repo,
        &["diff", "--no-color", "--no-ext-diff", "-U3", base, head],
    )?;
    Ok(Change::parse(&names, &unified))
}

/// The contents at `head` of `paths`; a path git cannot show is absent at head.
pub fn head_files(
    repo: &Path,
    head: &Sha,
    paths: BTreeSet<RepoPath>,
) -> Result<HeadFiles, ShellError> {
    let mut files = BTreeMap::new();
    for path in paths {
        let object = format!("{}:{}", head.as_str(), path.as_str());
        let text = git(repo, &["show", &object])?.ok();
        files.insert(path, text);
    }
    Ok(HeadFiles::new(files))
}

/// For each query: whether its commit exists, holds its test file and, when it names one,
/// is on its branch.
pub fn proof_objects(repo: &Path, queries: BTreeSet<Query>) -> Result<ProofObjects, ShellError> {
    let mut found = BTreeMap::new();
    for query in queries {
        let commit = query.commit.as_str();
        let succeeds = |args: &[&str]| git(repo, args).map(|result| result.is_ok());
        let state = if !succeeds(&["cat-file", "-e", &format!("{commit}^{{commit}}")])? {
            Found::NoCommit
        } else if !succeeds(&[
            "cat-file",
            "-e",
            &format!("{commit}:{}", query.path.as_str()),
        ])? {
            Found::NoFile
        } else if let Some(branch) = &query.branch
            && !succeeds(&[
                "merge-base",
                "--is-ancestor",
                commit,
                &format!("refs/heads/{branch}"),
            ])?
        {
            Found::OffBranch
        } else {
            Found::Present
        };
        found.insert(query, state);
    }
    Ok(ProofObjects::new(found))
}

/// The proof's test as its commit holds it, read with `git show`.
pub fn test_source(repo: &Path, proof: &Proof) -> Result<Option<TestSource>, ShellError> {
    let (commit, path) = (proof.commit.as_str(), proof.test.path.as_str());
    let file = git(repo, &["show", &format!("{commit}:{path}")])?.ok();
    let diff = git_ok(
        repo,
        &[
            "show",
            "--format=",
            "--no-color",
            "--no-ext-diff",
            commit,
            "--",
            path,
        ],
    )?;
    Ok(proof::source(&proof.test, file.as_deref(), &diff))
}

pub fn read(path: &Path) -> Result<String, ShellError> {
    std::fs::read_to_string(path).map_err(|source| ShellError::Read {
        path: path.to_path_buf(),
        source,
    })
}

pub fn write(path: &Path, text: &str) -> Result<(), ShellError> {
    std::fs::write(path, text).map_err(|source| ShellError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Sorted files with `extension` directly in `dir`; none when `dir` does not exist.
pub fn files(dir: &Path, extension: &str) -> Result<Vec<PathBuf>, ShellError> {
    entries(dir, |path| {
        path.is_file() && path.extension().is_some_and(|ext| ext == extension)
    })
}

pub fn subdirectories(dir: &Path) -> Result<Vec<PathBuf>, ShellError> {
    entries(dir, Path::is_dir)
}

fn entries(dir: &Path, keep: impl Fn(&Path) -> bool) -> Result<Vec<PathBuf>, ShellError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let unreadable = |source| ShellError::Read {
        path: dir.to_path_buf(),
        source,
    };
    let mut paths = std::fs::read_dir(dir)
        .map_err(unreadable)?
        .map(|entry| entry.map(|entry| entry.path()).map_err(unreadable))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| keep(path));
    paths.sort();
    Ok(paths)
}
