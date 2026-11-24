//! `revy-review <command> [dir]`: the commands the revy-review skill runs.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use revy_review::cli;

const USAGE: &str = "\
Usage: revy-review <command> [dir]

Commands (dir: the review directory holding state.json; default: current directory):
  rules     validate the rules and plug-in reviewers, print the index for the change's languages
  check     validate every file in the review directory and the accounting across them
  sync      render report.md from a clean review, or read back the owner's edits
  comments  print the post-ready comments from report.md
";

#[derive(Debug, Clone, Copy)]
enum Command {
    Rules,
    Check,
    Sync,
    Comments,
}

#[derive(Debug)]
enum Invocation {
    Run(Command, PathBuf),
    Help,
    Version,
}

fn parse(args: &[OsString]) -> Result<Invocation, String> {
    let (name, rest) = match args {
        [] => return Err("missing command".to_owned()),
        [name, rest @ ..] => (name.to_string_lossy(), rest),
    };
    let command = match name.as_ref() {
        "-h" | "--help" | "help" => return Ok(Invocation::Help),
        "-V" | "--version" => return Ok(Invocation::Version),
        "rules" => Command::Rules,
        "check" => Command::Check,
        "sync" => Command::Sync,
        "comments" => Command::Comments,
        other => return Err(format!("unknown command `{other}`")),
    };
    match rest {
        [] => Ok(Invocation::Run(command, PathBuf::from("."))),
        [dir] => Ok(Invocation::Run(command, PathBuf::from(dir))),
        [_, extra, ..] => Err(format!("unexpected argument `{}`", extra.to_string_lossy())),
    }
}

fn run(command: Command, dir: &Path) -> ExitCode {
    match command {
        Command::Rules => cli::rules(dir),
        Command::Check => cli::check(dir),
        Command::Sync => cli::sync(dir),
        Command::Comments => cli::comments(dir),
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match parse(&args) {
        Ok(Invocation::Run(command, dir)) => run(command, &dir),
        Ok(Invocation::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Invocation::Version) => {
            println!("revy-review {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprint!("error: {message}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
