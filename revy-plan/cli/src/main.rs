use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use fs4::fs_std::FileExt;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use revy_plan::capability::CapabilityReportInput;
use revy_plan::document::ParsedPlan;
use revy_plan::ids::FindingId;
use revy_plan::packet::{
    ReadyPacket, RevisionResult, WorkKind, WorkSelector, packet_schema_by_name,
};
use revy_plan::patch::{PatchOperation, apply_all};
use revy_plan::primitives::{NonEmptyText, RepoAlias, Sha256Digest};
use revy_plan::render::{Appendix, RenderContext};
use revy_plan::repository::{Repositories, Repository};
use revy_plan::runtime::{Checkouts, TranscriptStores};
use revy_plan::state::{ExplorationProgress, ReopenTarget, RunState};

#[derive(Parser)]
#[command(
    name = "revy-plan",
    version,
    about = "revy-plan: evidence-first implementation planning"
)]
struct Cli {
    #[command(subcommand)]
    command: PlanCommand,
}

#[derive(Subcommand)]
enum PlanCommand {
    /// Create plan.json for a target repository from a capability report, and
    /// make it the current plan
    Init(InitArgs),
    /// Print the generated JSON Schema, one named section, or a packet's result schema
    Schema {
        /// Print one named definition instead of the full schema
        section: Option<String>,
        /// Print the result schema for one packet kind
        #[arg(long)]
        packet: Option<String>,
    },
    /// Rewrite plan.json in canonical ordering
    Format(PlanLocationArgs),
    /// Print the ready work frontier for the plan's current state
    Status(StatusArgs),
    /// Apply a checked work result, advancing the plan atomically
    Apply(ApplyArgs),
    /// Dry-run a work result: decode, runtime check, and transition, reporting
    /// every problem without writing the plan
    Check(CheckArgs),
    /// Succeed only when the plan has reached its final state
    Gate(GateArgs),
    /// Render the current plan to Markdown and standalone HTML
    Render(RenderArgs),
    /// Serve the rendered plan at one stable loopback URL
    Serve(ServeArgs),
    /// Send the plan back to exploration or drafting, keeping approved
    /// capabilities and carrying the set-aside design for `--patch`
    Reopen(ReopenArgs),
    /// Revise the candidate under review with an id-addressed patch, then
    /// re-enter review at the wave after the last recorded review round
    Revise(ReviseArgs),
    /// Manage the shared, content-addressed transcript store
    Transcript {
        #[command(subcommand)]
        command: TranscriptCommand,
    },
}

#[derive(Debug, Clone, Args)]
struct InitArgs {
    /// The primary repository: it keys the plan home and is cited unprefixed
    #[arg(long)]
    target: PathBuf,
    #[arg(long)]
    slug: String,
    /// The capability report the plan opens on, as JSON; its `repository` is
    /// filled from the captured checkout
    #[arg(long)]
    capabilities: PathBuf,
    /// The primary repository's alias (default: derived from its directory name)
    #[arg(long)]
    alias: Option<String>,
    #[command(flatten)]
    repos: RepoArgs,
}

#[derive(Debug, Clone, Args)]
struct RepoArgs {
    /// Another repository the plan cites as `ALIAS:path`; repeatable
    #[arg(long = "repo", value_name = "ALIAS=PATH")]
    repos: Vec<String>,
}

impl RepoArgs {
    /// Each `ALIAS=PATH`, parsed and captured.
    fn capture(&self, now: &str) -> Result<Vec<(RepoAlias, Repository)>, String> {
        self.repos
            .iter()
            .map(|raw| {
                let (alias, path) = raw
                    .split_once('=')
                    .ok_or_else(|| format!("--repo `{raw}`: expected ALIAS=PATH"))?;
                let alias =
                    RepoAlias::parse(alias).map_err(|error| format!("--repo `{raw}`: {error}"))?;
                let repository = revy_plan::repository::capture(Path::new(path), now)
                    .map_err(|error| format!("--repo `{raw}`: {error}"))?;
                Ok((alias, repository))
            })
            .collect()
    }
}

#[derive(Subcommand)]
enum TranscriptCommand {
    /// Copy FILE into `$REVY_PLAN_HOME/transcripts/<sha256>` and print the
    /// `{path, digest}` reference to cite as a runtime evidence `artifact`
    Add { file: PathBuf },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ReopenTargetArg {
    Exploration,
    Draft,
}

impl From<ReopenTargetArg> for ReopenTarget {
    fn from(target: ReopenTargetArg) -> Self {
        match target {
            ReopenTargetArg::Exploration => Self::Exploration,
            ReopenTargetArg::Draft => Self::Draft,
        }
    }
}

#[derive(Debug, Clone, Args)]
struct ReopenArgs {
    /// What to reopen
    #[arg(value_enum, value_name = "TARGET")]
    stage: ReopenTargetArg,
    /// Why the plan goes back; shown in `status` and the render timeline
    #[arg(long)]
    reason: String,
    /// With `exploration`: also declare these repositories
    #[command(flatten)]
    repos: RepoArgs,
    /// With `exploration`: re-capture every declared repository's identity
    /// (after its checkout moved) and clear the carried evidence's `target`s,
    /// so the next exploration re-verifies and re-stamps them
    #[arg(long)]
    recapture: bool,
    #[command(flatten)]
    location: PlanLocationArgs,
}

#[derive(Debug, Clone, Args)]
struct ReviseArgs {
    /// RFC 6902 ops over `{"candidate": <current candidate>}` (`-` reads stdin);
    /// `@<id>` segments select array elements by id/subject/symbol/lane
    #[arg(long)]
    patch: PathBuf,
    /// What the revision changes and why
    #[arg(long)]
    note: String,
    /// Review findings (any severity) this revision addresses
    #[arg(long, num_args = 1..)]
    addresses: Vec<String>,
    /// Dry run: report every problem without writing the plan
    #[arg(long)]
    check: bool,
    #[command(flatten)]
    location: PlanLocationArgs,
}

/// Where the plan lives: `--plan`, or `--target` with `--slug`, or else the
/// current plan `init` recorded.
#[derive(Debug, Clone, Args)]
struct PlanLocationArgs {
    #[arg(long)]
    plan: Option<PathBuf>,
    #[arg(long)]
    target: Option<PathBuf>,
    #[arg(long)]
    slug: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum StatusFormat {
    Json,
    Pretty,
}

#[derive(Debug, Clone, Args)]
struct StatusArgs {
    #[arg(long, value_enum, default_value_t = StatusFormat::Json)]
    format: StatusFormat,
    /// Inline each packet's result schema
    #[arg(long)]
    schema: bool,
    #[command(flatten)]
    location: PlanLocationArgs,
}

#[derive(Debug, Clone, Args)]
struct ResultArgs {
    /// The work result, as JSON matching the packet schema (`-` reads stdin)
    #[arg(long, conflicts_with = "patch", required_unless_present = "patch")]
    result: Option<PathBuf>,
    /// A JSON array of RFC 6902 add/replace/remove ops over the packet's subject
    /// (`-` reads stdin)
    #[arg(long)]
    patch: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
struct ApplyArgs {
    /// The work key or packet kind the result answers, as reported by `status`;
    /// optional when exactly one packet is ready
    #[arg(long)]
    work: Option<String>,
    #[command(flatten)]
    input: ResultArgs,
    #[command(flatten)]
    location: PlanLocationArgs,
}

#[derive(Debug, Clone, Args)]
struct CheckArgs {
    /// The work key or packet kind the result answers; optional when exactly one
    /// packet is ready
    #[arg(long, visible_alias = "packet")]
    work: Option<String>,
    #[command(flatten)]
    input: ResultArgs,
    #[command(flatten)]
    location: PlanLocationArgs,
}

#[derive(Debug, Clone, Args)]
struct GateArgs {
    /// Pass a final plan even when it carries accepted unresolved findings
    #[arg(long)]
    allow_accepted: bool,
    #[command(flatten)]
    location: PlanLocationArgs,
}

#[derive(Debug, Clone, Args)]
struct RenderArgs {
    #[arg(long)]
    out: Option<PathBuf>,
    /// Omit the derived appendix (evidence, impact, symbols, call graph, timeline, discussion)
    #[arg(long)]
    brief: bool,
    #[command(flatten)]
    location: PlanLocationArgs,
}

#[derive(Debug, Clone, Args)]
struct ServeArgs {
    #[arg(long, default_value_t = 7777)]
    port: u16,
    /// Omit the derived appendix (evidence, impact, symbols, call graph, timeline, discussion)
    #[arg(long)]
    brief: bool,
    #[command(flatten)]
    location: PlanLocationArgs,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        PlanCommand::Schema { section, packet } => {
            run_schema(section.as_deref(), packet.as_deref())
        }
        PlanCommand::Init(args) => run_init(&args),
        PlanCommand::Format(location) => run_format(&location),
        PlanCommand::Status(args) => run_status(&args),
        PlanCommand::Apply(args) => run_apply(&args),
        PlanCommand::Check(args) => run_check(&args),
        PlanCommand::Gate(args) => run_gate(&args),
        PlanCommand::Render(args) => run_render(&args),
        PlanCommand::Serve(args) => run_serve(&args),
        PlanCommand::Reopen(args) => run_reopen(&args),
        PlanCommand::Revise(args) => run_revise(&args),
        PlanCommand::Transcript {
            command: TranscriptCommand::Add { file },
        } => run_transcript_add(&file),
    }
}

fn current_timestamp() -> Result<String, String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| error.to_string())
}

fn resolve_plan_path(location: &PlanLocationArgs) -> Result<PathBuf, String> {
    if let Some(plan) = &location.plan {
        return Ok(plan.clone());
    }
    match (location.target.as_deref(), location.slug.as_deref()) {
        (Some(target), Some(raw_slug)) => {
            let slug =
                revy_plan::primitives::Slug::parse(raw_slug).map_err(|error| error.to_string())?;
            let repository =
                revy_plan::repository::resolve(target).map_err(|error| error.to_string())?;
            revy_plan::paths::plan_path(&repository.name, &slug).map_err(|error| error.to_string())
        }
        (Some(target), None) => {
            let repository = revy_plan::repository::resolve(target)
                .map_err(|error| format!("--target {}: {error}", target.display()))?;
            repo_current_plan_path(&repository)
        }
        (None, None) => current_plan_path(),
        (None, Some(_)) => Err("--slug needs --target; or pass --plan".to_owned()),
    }
}

/// The current plan for the repository holding the working directory, or —
/// outside any git repository — the plan `init` last recorded globally.
fn current_plan_path() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|error| format!("working directory: {error}"))?;
    if let Ok(repository) = revy_plan::repository::resolve(&cwd) {
        return repo_current_plan_path(&repository);
    }
    let file = revy_plan::paths::current_plan_file().map_err(|error| error.to_string())?;
    read_current(&file)?.ok_or_else(|| {
        format!(
            "no plan given, {} is not inside a git repository, and there is no global current plan ({} is absent); pass --plan",
            cwd.display(),
            file.display()
        )
    })
}

/// The plan `init` last recorded as current for `repository`.
fn repo_current_plan_path(
    repository: &revy_plan::repository::ResolvedRepository,
) -> Result<PathBuf, String> {
    let file = revy_plan::paths::repo_current_plan_file(&repository.name)
        .map_err(|error| error.to_string())?;
    read_current(&file)?.ok_or_else(|| {
        format!(
            "no plan given and no current plan for the repository at {} ({} is absent); pass --plan, or --target REPO --slug SLUG, or run `revy-plan init` for this repository",
            repository.root,
            file.display()
        )
    })
}

/// The plan path a current-plan file names; `None` when the file is absent.
fn read_current(file: &Path) -> Result<Option<PathBuf>, String> {
    match std::fs::read_to_string(file) {
        Ok(contents) if !contents.trim().is_empty() => Ok(Some(PathBuf::from(contents.trim()))),
        Ok(_) => Err(format!(
            "the current plan file {} is empty; pass --plan",
            file.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", file.display())),
    }
}

fn plan_path_or_exit(location: &PlanLocationArgs) -> Result<PathBuf, ExitCode> {
    resolve_plan_path(location).map_err(|error| {
        eprintln!("{error}");
        ExitCode::from(2)
    })
}

fn load_or_exit(path: &Path) -> Result<ParsedPlan, ExitCode> {
    revy_plan::artifact::load(path).map_err(|error| {
        eprintln!("{error}");
        ExitCode::from(2)
    })
}

// --- Locking -------------------------------------------------------------------

/// The lock sibling of a plan file: `<plan>.lock`.
fn lock_path(plan_path: &Path) -> PathBuf {
    let mut name = plan_path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// Run `body` while holding a file lock on `<plan>.lock` — exclusive for the
/// load→apply→save window of `apply`/`format`, shared for a read.
fn with_lock<T>(plan_path: &Path, exclusive: bool, body: impl FnOnce() -> T) -> Result<T, String> {
    let lock = lock_path(plan_path);
    if let Some(parent) = lock.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock)
        .map_err(|error| format!("{}: {error}", lock.display()))?;
    if exclusive {
        file.lock_exclusive()
    } else {
        file.lock_shared()
    }
    .map_err(|error| format!("{}: {error}", lock.display()))?;
    let result = body();
    let _ = FileExt::unlock(&file);
    Ok(result)
}

// --- Schema --------------------------------------------------------------------

fn run_schema(section: Option<&str>, packet: Option<&str>) -> ExitCode {
    if let Some(kind) = packet {
        return packet_schema_by_name(kind).map_or_else(
            || {
                eprintln!("unknown packet kind `{kind}`");
                ExitCode::from(2)
            },
            |value| {
                println!("{value:#}");
                ExitCode::SUCCESS
            },
        );
    }
    let Some(name) = section else {
        println!("{:#}", revy_plan::schema::full());
        return ExitCode::SUCCESS;
    };
    match revy_plan::schema::section(name) {
        Ok(value) => {
            println!("{value:#}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

// --- Init ----------------------------------------------------------------------

fn run_init(args: &InitArgs) -> ExitCode {
    match try_run_init(args) {
        Ok(path) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn try_run_init(args: &InitArgs) -> Result<PathBuf, String> {
    let capabilities = args.capabilities.as_path();
    let slug = revy_plan::primitives::Slug::parse(&args.slug).map_err(|error| error.to_string())?;
    let report_bytes = std::fs::read(capabilities)
        .map_err(|error| format!("{}: {error}", capabilities.display()))?;
    let report: CapabilityReportInput = serde_json::from_slice(&report_bytes)
        .map_err(|error| format!("{}: {error}", capabilities.display()))?;
    let now = current_timestamp()?;
    let repository =
        revy_plan::repository::capture(&args.target, &now).map_err(|error| error.to_string())?;
    let alias = match &args.alias {
        Some(raw) => RepoAlias::parse(raw).map_err(|error| format!("--alias `{raw}`: {error}"))?,
        None => RepoAlias::for_key(&repository.name),
    };
    let report = report.stamped(repository.identity());
    let path =
        revy_plan::paths::plan_path(&repository.name, &slug).map_err(|error| error.to_string())?;
    let destination_parent = path
        .parent()
        .ok_or_else(|| "plan path has no parent".to_owned())?;
    let repositories = Repositories::new(alias, repository, args.repos.capture(&now)?)
        .map_err(|error| error.to_string())?;
    for repo in repositories.iter() {
        revy_plan::paths::ensure_destination_parent_outside_target(
            destination_parent,
            Path::new(repo.repository.root.as_str()),
        )
        .map_err(|error| error.to_string())?;
    }
    let plan = ParsedPlan::new(slug, repositories, report);
    revy_plan::artifact::create_new(&path, &plan).map_err(|error| error.to_string())?;
    let pointer = format!("{}\n", path.display());
    let current = [
        revy_plan::paths::repo_current_plan_file(&plan.repository().name),
        revy_plan::paths::current_plan_file(),
    ];
    for file in current {
        let file = file.map_err(|error| error.to_string())?;
        revy_plan::artifact::save_text_atomic(&file, &pointer)
            .map_err(|error| error.to_string())?;
    }
    Ok(path)
}

// --- Format --------------------------------------------------------------------

fn run_format(location: &PlanLocationArgs) -> ExitCode {
    let path = match plan_path_or_exit(location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    let outcome = with_lock(&path, true, || -> Result<(), String> {
        let plan = revy_plan::artifact::load(&path).map_err(|error| error.to_string())?;
        revy_plan::artifact::save_atomic(&path, &plan).map_err(|error| error.to_string())
    });
    match outcome.and_then(|inner| inner) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

// --- Status --------------------------------------------------------------------

fn run_status(args: &StatusArgs) -> ExitCode {
    let path = match plan_path_or_exit(&args.location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    let loaded = with_lock(&path, false, || load_or_exit(&path));
    let plan = match loaded {
        Ok(Ok(plan)) => plan,
        Ok(Err(code)) => return code,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let object = status_object(&plan, args.schema);
    let rendered = match args.format {
        StatusFormat::Json => serde_json::to_string(&object),
        StatusFormat::Pretty => serde_json::to_string_pretty(&object),
    };
    match rendered {
        Ok(rendered) => {
            println!("{rendered}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("could not serialize the status: {error}");
            ExitCode::from(2)
        }
    }
}

/// The `status` object (design §7): the state, the wave where one exists, the
/// ready packets (each with its result schema inlined when `--schema` is set), and
/// the accepted-finding count on a final plan.
fn status_object(plan: &ParsedPlan, with_schema: bool) -> Value {
    let frontier = revy_plan::workflow::frontier(plan);
    let packets: Vec<Value> = frontier
        .iter()
        .map(|packet| {
            let mut value = serde_json::to_value(packet).unwrap_or_else(|_| json!({}));
            if with_schema && let Value::Object(map) = &mut value {
                map.insert("schema".to_owned(), packet.result_schema());
            }
            value
        })
        .collect();
    let mut object = json!({ "state": state_name(plan.state()), "packets": packets });
    if let Value::Object(map) = &mut object {
        if let Some(wave) = state_wave(plan.state()) {
            map.insert("wave".to_owned(), json!(wave));
        }
        if let RunState::Final { accepted, .. } = plan.state() {
            map.insert("accepted_findings".to_owned(), json!(accepted.len()));
        }
        if let Some(capabilities) = plan.state().capabilities() {
            map.insert(
                "capability_approval".to_owned(),
                json!(capabilities.approval().name()),
            );
        }
        let reopens = plan.state().reopens();
        if !reopens.is_empty() {
            map.insert(
                "reopens".to_owned(),
                json!(
                    reopens
                        .iter()
                        .map(|reopen| json!({ "target": reopen.target, "reason": reopen.reason }))
                        .collect::<Vec<_>>()
                ),
            );
        }
        if let Some(revision) = plan.state().revisions().last() {
            map.insert("latest_revision".to_owned(), revision_summary(revision));
        }
    }
    object
}

/// The latest revision as `status` shows it: wave, note, addresses, and —
/// when present — the escalation it answered and the user-decision changes.
fn revision_summary(revision: &revy_plan::state::Revision) -> Value {
    let mut value =
        json!({ "wave": revision.wave, "note": revision.note, "addresses": revision.addresses });
    if let Value::Object(map) = &mut value {
        if let Some(cause) = &revision.cause {
            map.insert("cause".to_owned(), json!(cause));
        }
        if !revision.decision_changes.is_empty() {
            map.insert(
                "decision_changes".to_owned(),
                json!(revision.decision_changes),
            );
        }
    }
    value
}

const fn state_name(state: &RunState) -> &'static str {
    match state {
        RunState::AwaitingCapabilityApproval { .. } => "awaiting_capability_approval",
        RunState::Exploring { .. } => "exploring",
        RunState::AwaitingUserDecision { .. } => "awaiting_user_decision",
        RunState::Drafting { .. } => "drafting",
        RunState::Reviewing { .. } => "reviewing",
        RunState::Correcting { .. } => "correcting",
        RunState::AwaitingReviewDecision { .. } => "awaiting_review_decision",
        RunState::Final { .. } => "final",
    }
}

const fn state_wave(state: &RunState) -> Option<u32> {
    match state {
        RunState::Reviewing { wave, .. }
        | RunState::Correcting { wave, .. }
        | RunState::AwaitingReviewDecision { wave, .. } => Some(wave.get()),
        _ => None,
    }
}

// --- Apply ---------------------------------------------------------------------

fn run_apply(args: &ApplyArgs) -> ExitCode {
    let path = match plan_path_or_exit(&args.location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    let input = match read_input(&args.input) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let outcome = with_lock(&path, true, || {
        try_apply(&path, args.work.as_deref(), &input)
    });
    report_outcome(outcome, ToOwned::to_owned)
}

/// Apply and save, returning the one line naming what was applied, to which
/// plan, and the state it entered — so a result applied to the wrong plan or
/// packet is visible at once.
fn try_apply(path: &Path, work: Option<&str>, input: &ResultInput) -> Result<String, String> {
    let plan = revy_plan::artifact::load(path).map_err(|error| error.to_string())?;
    let evaluated = evaluate(plan, work, input)?;
    revy_plan::artifact::save_atomic(path, &evaluated.plan).map_err(|error| error.to_string())?;
    Ok(applied_line(
        &format!("{} {}", evaluated.kind.name(), evaluated.key.as_str()),
        path,
        &evaluated.plan,
    ))
}

fn applied_line(what: &str, path: &Path, plan: &ParsedPlan) -> String {
    format!(
        "applied {what} to {} → {}",
        path.display(),
        state_label(plan.state())
    )
}

/// A result that applied: the advanced plan and the packet it answered.
struct Evaluated {
    plan: ParsedPlan,
    kind: WorkKind,
    key: revy_plan::packet::WorkKey,
}

fn run_check(args: &CheckArgs) -> ExitCode {
    let path = match plan_path_or_exit(&args.location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    let input = match read_input(&args.input) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let outcome = with_lock(&path, false, || -> Result<String, String> {
        let plan = revy_plan::artifact::load(&path).map_err(|error| error.to_string())?;
        let next = evaluate(plan, args.work.as_deref(), &input)?;
        Ok(state_name(next.plan.state()).to_owned())
    });
    match outcome {
        Ok(Ok(state)) => {
            println!("ok: the result applies cleanly; the plan would enter `{state}`");
            ExitCode::SUCCESS
        }
        Ok(Err(error)) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

/// The result a command was handed: a full result, or a patch over the
/// packet's subject. Read before the plan lock is taken, so stdin is consumed
/// once.
enum ResultInput {
    Result(Vec<u8>),
    Patch(Vec<u8>),
}

fn read_input(args: &ResultArgs) -> Result<ResultInput, String> {
    match (args.result.as_deref(), args.patch.as_deref()) {
        (Some(result), None) => read_source(result).map(ResultInput::Result),
        (None, Some(patch)) => read_source(patch).map(ResultInput::Patch),
        _ => Err("provide exactly one of --result or --patch".to_owned()),
    }
}

/// The bytes at `path`, or stdin when `path` is `-`.
fn read_source(path: &Path) -> Result<Vec<u8>, String> {
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut std::io::stdin(), &mut bytes)
            .map_err(|error| format!("stdin: {error}"))?;
        Ok(bytes)
    } else {
        std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
    }
}

/// Decode, runtime-check, and transition a result against the selected ready
/// packet, yielding the advanced plan or every problem found. Shared by
/// `apply` (which saves the plan) and `check` (which discards it).
fn evaluate(
    plan: ParsedPlan,
    work: Option<&str>,
    input: &ResultInput,
) -> Result<Evaluated, String> {
    let selector = WorkSelector::parse(work).map_err(|error| format!("--work: {error}"))?;
    let (key, kind, raw) = {
        let frontier = revy_plan::workflow::frontier(&plan);
        let packet = frontier
            .select(&selector)
            .map_err(|error| error.to_string())?;
        let bytes = match input {
            ResultInput::Result(bytes) => bytes.clone(),
            ResultInput::Patch(patch) => patched_result_bytes(&plan, packet, patch)?,
        };
        let raw = packet
            .decode_result(&bytes)
            .map_err(|error| error.to_string())?;
        (packet.key().clone(), packet.kind(), raw)
    };
    let (plan_transcripts, shared_transcripts) = transcript_dirs(&plan)?;
    let stores = TranscriptStores {
        plan: &plan_transcripts,
        shared: &shared_transcripts,
    };
    let checkouts = Checkouts::capture(plan.repositories(), &current_timestamp()?);
    match revy_plan::runtime::check_result(&plan, stores, &checkouts, raw) {
        Ok(checked) => revy_plan::workflow::apply(plan, &key, checked)
            .map(|plan| Evaluated { plan, kind, key })
            .map_err(|error| {
                rejection_report(
                    &[],
                    &error
                        .reasons()
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                )
            }),
        Err(rejection) => {
            let (runtime, provisional) = rejection.into_parts();
            let transition = revy_plan::workflow::rehearse(&plan, &key, provisional).err();
            Err(combined_report(&runtime, transition.as_ref()))
        }
    }
}

/// The runtime-check problems and the rehearsed transition's, dropping the
/// transition's "no target" report for evidence the runtime check already
/// reported as unstampable (a moved or uncapturable checkout).
fn combined_report(
    runtime: &revy_plan::issues::Issues<revy_plan::runtime::CheckError>,
    transition: Option<&revy_plan::workflow::Rejection>,
) -> String {
    let unstamped: std::collections::BTreeSet<&revy_plan::ids::EvidenceId> = runtime
        .iter()
        .flat_map(|issue| issue.error().unstamped_evidence())
        .collect();
    let runtime_lines: Vec<String> = runtime.iter().map(ToString::to_string).collect();
    let transition_lines: Vec<String> = transition
        .into_iter()
        .flat_map(revy_plan::issues::Issues::iter)
        .filter(|issue| {
            issue
                .error()
                .ungrounded_evidence()
                .is_none_or(|id| !unstamped.contains(id))
        })
        .map(ToString::to_string)
        .collect();
    rejection_report(&runtime_lines, &transition_lines)
}

/// Every runtime-check and transition problem, one per line, each with the
/// JSON pointer into the result it concerns.
fn rejection_report(runtime: &[String], transition: &[String]) -> String {
    let total = runtime.len() + transition.len();
    let mut lines = vec![format!(
        "result rejected ({total} problem{}):",
        if total == 1 { "" } else { "s" }
    )];
    lines.extend(runtime.iter().map(|line| format!("  - [runtime] {line}")));
    lines.extend(
        transition
            .iter()
            .map(|line| format!("  - [transition] {line}")),
    );
    lines.join("\n")
}

/// The result JSON built by patching the packet's subject document.
fn patched_result_bytes(
    plan: &ParsedPlan,
    packet: &ReadyPacket,
    patch_bytes: &[u8],
) -> Result<Vec<u8>, String> {
    let mut document = subject_document(plan, packet.kind())?;
    let operations: Vec<PatchOperation> =
        serde_json::from_slice(patch_bytes).map_err(|error| format!("--patch: {error}"))?;
    apply_all(&mut document, &operations).map_err(|error| error.to_string())?;
    serde_json::to_vec(&document).map_err(|error| error.to_string())
}

/// The document a `--patch` applies to (design §10): the prior exploration for an
/// `exploration` packet, the current candidate for a `correction`, and — after a
/// reopen — the prior brief and decisions for a `design_brief`, the prior draft
/// for a `draft`. Other kinds are not patchable.
fn subject_document(plan: &ParsedPlan, kind: WorkKind) -> Result<Value, String> {
    match (kind, plan.state()) {
        (
            WorkKind::Exploration,
            RunState::Exploring {
                progress: ExplorationProgress::Collecting { prior },
                ..
            },
        ) => {
            let mut value = serde_json::to_value(prior.raw()).map_err(|error| error.to_string())?;
            if let Value::Object(map) = &mut value {
                map.insert("next".to_owned(), json!({ "kind": "complete" }));
            }
            Ok(value)
        }
        (WorkKind::Correction, RunState::Correcting { candidate, .. }) => {
            Ok(json!({ "candidate": candidate.raw(), "triage": [] }))
        }
        (
            WorkKind::DesignBrief,
            RunState::Exploring {
                lineage:
                    revy_plan::state::DesignLineage {
                        prior_design: Some(prior),
                        ..
                    },
                ..
            },
        ) => Ok(json!({ "brief": prior.brief, "decisions": prior.decisions })),
        (
            WorkKind::Draft,
            RunState::Drafting {
                lineage:
                    revy_plan::state::DesignLineage {
                        prior_design: Some(prior),
                        ..
                    },
                ..
            },
        ) => prior
            .draft
            .as_ref()
            .map(|draft| json!({ "content": draft.content, "delivery": draft.delivery }))
            .ok_or_else(|| {
                "the prior design carries no draft to patch; supply --result".to_owned()
            }),
        _ => Err(format!(
            "packet kind `{}` is not patchable here (patchable: exploration, correction, and after a reopen design_brief and draft); supply --result",
            kind.name()
        )),
    }
}

/// The plan's own transcripts directory and the shared store.
fn transcript_dirs(plan: &ParsedPlan) -> Result<(PathBuf, PathBuf), String> {
    let own = revy_plan::paths::transcripts_dir(&plan.repository().name, plan.slug())
        .map_err(|error| error.to_string())?;
    let shared = revy_plan::paths::shared_transcripts_dir().map_err(|error| error.to_string())?;
    Ok((own, shared))
}

// --- Gate ----------------------------------------------------------------------

fn run_gate(args: &GateArgs) -> ExitCode {
    let path = match plan_path_or_exit(&args.location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    let loaded = with_lock(&path, false, || load_or_exit(&path));
    let plan = match loaded {
        Ok(Ok(plan)) => plan,
        Ok(Err(code)) => return code,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    match plan.state() {
        RunState::Final { accepted, .. } if accepted.is_empty() || args.allow_accepted => {
            ExitCode::SUCCESS
        }
        RunState::Final { accepted, .. } => {
            eprintln!(
                "plan is final but carries {} accepted unresolved finding(s); pass --allow-accepted to accept them",
                accepted.len()
            );
            for finding in accepted {
                eprintln!(
                    "  - [{:?}] {}: {}",
                    finding.severity, finding.id, finding.statement
                );
            }
            ExitCode::from(1)
        }
        other => {
            eprintln!(
                "plan has not reached its final state (state: {})",
                state_name(other)
            );
            ExitCode::from(1)
        }
    }
}

// --- Render --------------------------------------------------------------------

fn run_render(args: &RenderArgs) -> ExitCode {
    let path = match plan_path_or_exit(&args.location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    let outcome = with_lock(&path, false, || try_render(&path, args));
    match outcome {
        Ok(Ok((markdown, html))) => {
            println!("{}", markdown.display());
            println!("{}", html.display());
            ExitCode::SUCCESS
        }
        Ok(Err(error)) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn try_render(path: &Path, args: &RenderArgs) -> Result<(PathBuf, PathBuf), String> {
    let plan = revy_plan::artifact::load(path).map_err(|error| error.to_string())?;
    let preview;
    let (candidate, label) = if let Some(candidate) = plan.state().candidate() {
        (candidate, state_label(plan.state()))
    } else {
        preview = revy_plan::workflow::reopened_preview(plan.state()).ok_or_else(|| {
            format!(
                "render requires a candidate: the plan is `{}`, and no draft set aside by a reopen is retained to preview",
                state_name(plan.state())
            )
        })?;
        (&preview, "reopened".to_owned())
    };
    let context = RenderContext::for_state(plan.state(), &label);
    let rendered = revy_plan::render::render(candidate, &context, appendix_for(args.brief));
    let directory = args.out.clone().unwrap_or_else(|| {
        path.parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    });
    let markdown_path = directory.join("plan.md");
    let html_path = directory.join("plan.html");
    revy_plan::artifact::save_text_atomic(&markdown_path, &rendered.markdown)
        .map_err(|error| error.to_string())?;
    revy_plan::artifact::save_text_atomic(&html_path, &rendered.html)
        .map_err(|error| error.to_string())?;
    Ok((markdown_path, html_path))
}

const fn appendix_for(brief: bool) -> Appendix {
    if brief {
        Appendix::Omitted
    } else {
        Appendix::Included
    }
}

fn state_label(state: &RunState) -> String {
    match state {
        RunState::Reviewing { wave, .. } => format!("reviewing · wave {}", wave.get()),
        RunState::Correcting { wave, .. } => format!("correcting · wave {}", wave.get()),
        RunState::AwaitingReviewDecision { wave, .. } => {
            format!("awaiting review decision · wave {}", wave.get())
        }
        other => state_name(other).to_owned(),
    }
}

// --- Serve ---------------------------------------------------------------------

fn run_serve(args: &ServeArgs) -> ExitCode {
    let plan_path = match plan_path_or_exit(&args.location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    match revy_plan::serve::run(&plan_path, args.port, appendix_for(args.brief)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

// --- Reopen --------------------------------------------------------------------

fn run_reopen(args: &ReopenArgs) -> ExitCode {
    let path = match plan_path_or_exit(&args.location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    let Ok(reason) = NonEmptyText::parse(&args.reason) else {
        eprintln!("--reason must not be empty");
        return ExitCode::from(2);
    };
    let target = ReopenTarget::from(args.stage);
    if (!args.repos.repos.is_empty() || args.recapture) && target != ReopenTarget::Exploration {
        eprintln!("--repo and --recapture apply only to `reopen exploration`");
        return ExitCode::from(2);
    }
    let now = match current_timestamp() {
        Ok(now) => now,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let declared = match args.repos.capture(&now) {
        Ok(declared) => declared,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let outcome = with_lock(&path, true, || -> Result<String, String> {
        let plan = revy_plan::artifact::load(&path).map_err(|error| error.to_string())?;
        let plan = declared
            .into_iter()
            .try_fold(plan, |plan, (alias, repository)| {
                plan.declaring(alias, repository)
            })
            .map_err(|error| error.to_string())?;
        let next = if args.recapture {
            let plan = plan
                .recapturing(|repository| {
                    revy_plan::repository::capture(Path::new(repository.root.as_str()), &now)
                })
                .map_err(|error| format!("--recapture: {error}"))?;
            revy_plan::workflow::reopen_recaptured(plan, reason)
        } else {
            revy_plan::workflow::reopen(plan, target, reason)
        }
        .map_err(|error| error.to_string())?;
        revy_plan::artifact::save_atomic(&path, &next).map_err(|error| error.to_string())?;
        Ok(applied_line(
            &format!("reopen {}", target.name()),
            &path,
            &next,
        ))
    });
    report_outcome(outcome, ToOwned::to_owned)
}

/// Print a command's success line, or its error with exit 1 (rejected) or 2
/// (could not run).
fn report_outcome(
    outcome: Result<Result<String, String>, String>,
    success: impl FnOnce(&str) -> String,
) -> ExitCode {
    match outcome {
        Ok(Ok(value)) => {
            println!("{}", success(&value));
            ExitCode::SUCCESS
        }
        Ok(Err(error)) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

// --- Revise --------------------------------------------------------------------

fn run_revise(args: &ReviseArgs) -> ExitCode {
    let plan_path = match plan_path_or_exit(&args.location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    let parsed = (|| -> Result<(Vec<u8>, NonEmptyText, Vec<FindingId>), String> {
        let patch = read_source(&args.patch)?;
        let note = NonEmptyText::parse(&args.note).map_err(|_| "--note must not be empty")?;
        let addresses = args
            .addresses
            .iter()
            .map(|raw| {
                FindingId::parse(raw).map_err(|error| format!("--addresses `{raw}`: {error}"))
            })
            .collect::<Result<_, _>>()?;
        Ok((patch, note, addresses))
    })();
    let (patch, note, addresses) = match parsed {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let dry_run = args.check;
    let outcome = with_lock(&plan_path, !dry_run, || -> Result<String, String> {
        let plan = revy_plan::artifact::load(&plan_path).map_err(|error| error.to_string())?;
        let next = evaluate_revision(plan, &patch, note, addresses)?;
        if dry_run {
            return Ok(format!(
                "ok: the revision applies cleanly; the plan would enter `{}`",
                state_label(next.state())
            ));
        }
        revy_plan::artifact::save_atomic(&plan_path, &next).map_err(|error| error.to_string())?;
        Ok(applied_line("revise", &plan_path, &next))
    });
    report_outcome(outcome, ToOwned::to_owned)
}

/// Patch the current candidate, runtime-check its evidence, and transition,
/// yielding the revised plan or every problem found — the same report `check`
/// and `apply` give.
fn evaluate_revision(
    plan: ParsedPlan,
    patch: &[u8],
    note: NonEmptyText,
    addresses: Vec<FindingId>,
) -> Result<ParsedPlan, String> {
    let candidate = plan.state().candidate().ok_or_else(|| {
        format!(
            "revise applies to reviewing, correcting, awaiting_review_decision, or final; the plan is `{}`",
            state_name(plan.state())
        )
    })?;
    let mut document = json!({ "candidate": candidate.raw() });
    let operations: Vec<PatchOperation> =
        serde_json::from_slice(patch).map_err(|error| format!("--patch: {error}"))?;
    apply_all(&mut document, &operations).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&document).map_err(|error| error.to_string())?;
    let revision =
        RevisionResult::decode(&bytes, note, addresses).map_err(|error| error.to_string())?;
    let (plan_transcripts, shared_transcripts) = transcript_dirs(&plan)?;
    let stores = TranscriptStores {
        plan: &plan_transcripts,
        shared: &shared_transcripts,
    };
    let checkouts = Checkouts::capture(plan.repositories(), &current_timestamp()?);
    match revy_plan::runtime::check_revision(&plan, stores, &checkouts, revision) {
        Ok(checked) => revy_plan::workflow::revise(plan, checked).map_err(|error| {
            rejection_report(
                &[],
                &error
                    .reasons()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            )
        }),
        Err(rejection) => {
            let (runtime, provisional) = rejection.into_parts();
            let transition = revy_plan::workflow::rehearse_revision(&plan, provisional).err();
            Err(combined_report(&runtime, transition.as_ref()))
        }
    }
}

// --- Transcript ----------------------------------------------------------------

fn run_transcript_add(file: &Path) -> ExitCode {
    match try_transcript_add(file) {
        Ok(reference) => {
            println!("{reference}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

/// Store `file` under its digest in the shared store (idempotent) and return the
/// `TranscriptRef` JSON citing it: the file name as `path`, its digest.
fn try_transcript_add(file: &Path) -> Result<Value, String> {
    let bytes = std::fs::read(file).map_err(|error| format!("{}: {error}", file.display()))?;
    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{}: the file name is not valid UTF-8", file.display()))?;
    let path = revy_plan::primitives::RepoRelPath::parse(name).map_err(|error| {
        format!(
            "{}: the file name is not a usable path: {error}",
            file.display()
        )
    })?;
    let digest = Sha256Digest::hash(&bytes);
    let store = revy_plan::paths::shared_transcripts_dir().map_err(|error| error.to_string())?;
    let stored = store.join(digest.as_str());
    let present = std::fs::read(&stored).is_ok_and(|existing| existing == bytes);
    if !present {
        revy_plan::artifact::save_bytes_atomic(&stored, &bytes)
            .map_err(|error| error.to_string())?;
    }
    Ok(json!({ "path": path, "digest": digest }))
}
