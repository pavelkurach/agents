use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use fs4::fs_std::FileExt;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use revy_plan::capability::CapabilityReport;
use revy_plan::document::ParsedPlan;
use revy_plan::model::review::Finding;
use revy_plan::packet::{RawWorkResult, WorkKind, packet_schema_by_name};
use revy_plan::patch::{PatchOperation, apply_all};
use revy_plan::render::RenderContext;
use revy_plan::state::{ExplorationProgress, ReviewRound, RunState};

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
    /// Create plan.json for a target repository from a capability report
    Init {
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        slug: String,
        /// The capability report the plan opens on, as JSON
        #[arg(long)]
        capabilities: PathBuf,
    },
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
    /// Succeed only when the plan has reached its final state
    Gate(GateArgs),
    /// Render the current plan to Markdown and standalone HTML
    Render(RenderArgs),
    /// Serve the rendered plan at one stable loopback URL
    Serve(ServeArgs),
}

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
struct ApplyArgs {
    /// The work result to apply, as JSON matching the packet schema
    #[arg(long)]
    result: Option<PathBuf>,
    /// A JSON array of RFC 6902 add/replace/remove ops over the packet's subject
    #[arg(long)]
    patch: Option<PathBuf>,
    /// The work key the result answers, as reported by `status`
    #[arg(long)]
    work: String,
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
    #[command(flatten)]
    location: PlanLocationArgs,
}

#[derive(Debug, Clone, Args)]
struct ServeArgs {
    #[arg(long, default_value_t = 7777)]
    port: u16,
    #[command(flatten)]
    location: PlanLocationArgs,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        PlanCommand::Schema { section, packet } => {
            run_schema(section.as_deref(), packet.as_deref())
        }
        PlanCommand::Init {
            target,
            slug,
            capabilities,
        } => run_init(&target, &slug, &capabilities),
        PlanCommand::Format(location) => run_format(&location),
        PlanCommand::Status(args) => run_status(&args),
        PlanCommand::Apply(args) => run_apply(&args),
        PlanCommand::Gate(args) => run_gate(&args),
        PlanCommand::Render(args) => run_render(&args),
        PlanCommand::Serve(args) => run_serve(&args),
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
        _ => Err("provide --plan or both --target and --slug".to_owned()),
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

fn run_init(target: &Path, slug: &str, capabilities: &Path) -> ExitCode {
    match try_run_init(target, slug, capabilities) {
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

fn try_run_init(target: &Path, raw_slug: &str, capabilities: &Path) -> Result<PathBuf, String> {
    let slug = revy_plan::primitives::Slug::parse(raw_slug).map_err(|error| error.to_string())?;
    let report_bytes = std::fs::read(capabilities)
        .map_err(|error| format!("{}: {error}", capabilities.display()))?;
    let report: CapabilityReport = serde_json::from_slice(&report_bytes)
        .map_err(|error| format!("{}: {error}", capabilities.display()))?;
    let now = current_timestamp()?;
    let repository =
        revy_plan::repository::capture(target, &now).map_err(|error| error.to_string())?;
    let path =
        revy_plan::paths::plan_path(&repository.name, &slug).map_err(|error| error.to_string())?;
    let destination_parent = path
        .parent()
        .ok_or_else(|| "plan path has no parent".to_owned())?;
    revy_plan::paths::ensure_destination_parent_outside_target(
        destination_parent,
        Path::new(repository.root.as_str()),
    )
    .map_err(|error| error.to_string())?;
    let plan = ParsedPlan::new(slug, repository, report);
    revy_plan::artifact::create_new(&path, &plan).map_err(|error| error.to_string())?;
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
    }
    object
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
    let outcome = with_lock(&path, true, || try_apply(&path, args));
    match outcome {
        Ok(Ok(())) => ExitCode::SUCCESS,
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

fn try_apply(path: &Path, args: &ApplyArgs) -> Result<(), String> {
    let plan = revy_plan::artifact::load(path).map_err(|error| error.to_string())?;
    let bytes = result_bytes(
        &plan,
        &args.work,
        args.result.as_deref(),
        args.patch.as_deref(),
    )?;
    let next = apply_result_bytes(plan, &args.work, &bytes)?;
    revy_plan::artifact::save_atomic(path, &next).map_err(|error| error.to_string())
}

/// The result JSON to apply: read from `--result`, or built by patching the
/// packet's subject document with `--patch`.
fn result_bytes(
    plan: &ParsedPlan,
    work_key: &str,
    result: Option<&Path>,
    patch: Option<&Path>,
) -> Result<Vec<u8>, String> {
    match (result, patch) {
        (Some(result), None) => {
            std::fs::read(result).map_err(|error| format!("{}: {error}", result.display()))
        }
        (None, Some(patch)) => patched_result_bytes(plan, work_key, patch),
        _ => Err("provide exactly one of --result or --patch".to_owned()),
    }
}

fn patched_result_bytes(
    plan: &ParsedPlan,
    work_key: &str,
    patch: &Path,
) -> Result<Vec<u8>, String> {
    let kind = {
        let frontier = revy_plan::workflow::frontier(plan);
        frontier
            .iter()
            .find(|packet| packet.key().as_str() == work_key)
            .map(revy_plan::packet::ReadyPacket::kind)
            .ok_or_else(|| format!("no ready packet offers work key `{work_key}`"))?
    };
    let mut document = subject_document(plan, kind)?;
    let patch_bytes =
        std::fs::read(patch).map_err(|error| format!("{}: {error}", patch.display()))?;
    let operations: Vec<PatchOperation> = serde_json::from_slice(&patch_bytes)
        .map_err(|error| format!("{}: {error}", patch.display()))?;
    apply_all(&mut document, &operations).map_err(|error| error.to_string())?;
    serde_json::to_vec(&document).map_err(|error| error.to_string())
}

/// The document a `--patch` applies to (design §10): the prior exploration for an
/// `exploration` packet, the current candidate for a `correction`. Other kinds are
/// not patchable.
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
        _ => Err(format!(
            "packet kind `{}` is not patchable; supply --result",
            kind.name()
        )),
    }
}

fn apply_result_bytes(
    plan: ParsedPlan,
    work_key: &str,
    bytes: &[u8],
) -> Result<ParsedPlan, String> {
    let (key, raw): (_, RawWorkResult) = {
        let frontier = revy_plan::workflow::frontier(&plan);
        let packet = frontier
            .iter()
            .find(|packet| packet.key().as_str() == work_key)
            .ok_or_else(|| format!("no ready packet offers work key `{work_key}`"))?;
        let raw = packet
            .decode_result(bytes)
            .map_err(|error| error.to_string())?;
        (packet.key().clone(), raw)
    };
    let transcripts_root = transcripts_root(&plan)?;
    let checked = revy_plan::runtime::check_result(&plan, &transcripts_root, raw)
        .map_err(|error| error.to_string())?;
    revy_plan::workflow::apply(plan, &key, checked).map_err(|error| error.to_string())
}

fn transcripts_root(plan: &ParsedPlan) -> Result<PathBuf, String> {
    revy_plan::paths::transcripts_dir(&plan.repository().name, plan.slug())
        .map_err(|error| error.to_string())
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
    let candidate = plan
        .state()
        .candidate()
        .ok_or_else(|| "render requires a state that carries a candidate".to_owned())?;
    let label = state_label(plan.state());
    let context = RenderContext {
        state_label: &label,
        is_final: matches!(plan.state(), RunState::Final { .. }),
        history: state_history(plan.state()),
        accepted: state_accepted(plan.state()),
        comments: plan.state().comments(),
    };
    let rendered = revy_plan::render::render(candidate, &context);
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

fn state_history(state: &RunState) -> &[ReviewRound] {
    match state {
        RunState::Reviewing { history, .. }
        | RunState::Correcting { history, .. }
        | RunState::AwaitingReviewDecision { history, .. }
        | RunState::Final { history, .. } => history,
        _ => &[],
    }
}

fn state_accepted(state: &RunState) -> &[Finding] {
    match state {
        RunState::Final { accepted, .. } => accepted,
        _ => &[],
    }
}

// --- Serve ---------------------------------------------------------------------

fn run_serve(args: &ServeArgs) -> ExitCode {
    let plan_path = match plan_path_or_exit(&args.location) {
        Ok(path) => path,
        Err(code) => return code,
    };
    match revy_plan::serve::run(&plan_path, args.port) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
