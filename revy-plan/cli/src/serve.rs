//! The stable loopback server for the rendered plan.
//!
//! One URL serves the current plan's HTML, re-rendered from disk on each request.
//! Any candidate-bearing state renders (a non-final one carries a draft
//! watermark); a state with no candidate is reported as a diagnostic page.
//!
//! The served page also carries render.py's `--serve` comment affordance: a
//! per-block "comment" button posts to `/comment`, and the comment is appended
//! to the plan's typed review overlay. A version stamp (the digest of the plan
//! file the page was rendered from) guards against commenting on a page that no
//! longer matches the plan on disk.

use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use fs4::fs_std::FileExt;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::artifact::{self, ArtifactError};
use crate::document::ParsedPlan;
use crate::model::discussion::BlockAnchor;
use crate::primitives::{NonEmptyText, Sha256Digest};
use crate::render::{self, Appendix, RenderContext};
use crate::state::RunState;

#[derive(Debug, Error)]
pub enum ServeError {
    #[error("could not bind http://127.0.0.1:{port}/: {source}")]
    Bind {
        port: u16,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("could not print the serve URL: {0}")]
    Print(#[source] std::io::Error),
    #[error("could not construct the HTML content type header")]
    ContentTypeHeader,
    #[error("the server's accept loop failed: {0}")]
    Accept(#[source] std::io::Error),
}

/// Why the current plan cannot be served as a rendered page.
#[derive(Debug, Error)]
enum PageError {
    #[error("{0}")]
    Artifact(#[from] ArtifactError),
    #[error("the plan has not reached a state that carries a candidate to render")]
    NoCandidate,
}

impl PageError {
    const fn code(&self) -> &'static str {
        match self {
            Self::Artifact(ArtifactError::Parse { .. }) => "structure-invalid",
            Self::Artifact(_) => "plan-unreadable",
            Self::NoCandidate => "plan-no-candidate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    modified: SystemTime,
    size: u64,
    digest: [u8; 32],
}

impl FileStamp {
    fn read(path: &Path) -> Result<Self, std::io::Error> {
        let metadata = std::fs::metadata(path)?;
        let bytes = std::fs::read(path)?;
        Ok(Self {
            modified: metadata.modified()?,
            size: metadata.len(),
            digest: Sha256::digest(bytes).into(),
        })
    }
}

#[derive(Debug)]
struct Cache {
    appendix: Appendix,
    stamp: Option<FileStamp>,
    html: Option<String>,
}

impl Cache {
    const fn new(appendix: Appendix) -> Self {
        Self {
            appendix,
            stamp: None,
            html: None,
        }
    }

    fn response(&mut self, path: &Path) -> String {
        let stamp = match FileStamp::read(path) {
            Ok(stamp) => stamp,
            Err(error) => return diagnostic_html("plan-unreadable", &error.to_string()),
        };

        if self.stamp.as_ref() == Some(&stamp)
            && let Some(html) = &self.html
        {
            return html.clone();
        }

        match render_plan(path, self.appendix) {
            Ok(html) => {
                self.stamp = Some(stamp);
                self.html = Some(html.clone());
                html
            }
            Err(error) => diagnostic_html(error.code(), &error.to_string()),
        }
    }
}

/// Render the current plan's HTML, or fail with the reason it cannot be served.
/// Any candidate-bearing state renders (a non-final one carries a draft
/// watermark); only a state with no candidate cannot be shown. The returned page
/// carries the comment affordance, stamped with the digest of the exact bytes it
/// was rendered from so a later comment can detect staleness.
fn render_plan(path: &Path, appendix: Appendix) -> Result<String, PageError> {
    let bytes = std::fs::read(path).map_err(|source| {
        PageError::Artifact(ArtifactError::Io {
            path: path.display().to_string(),
            source,
        })
    })?;
    let plan = ParsedPlan::parse(&bytes).map_err(|source| ArtifactError::Parse {
        path: path.display().to_string(),
        source,
    })?;
    let candidate = plan.state().candidate().ok_or(PageError::NoCandidate)?;
    let label = state_label(plan.state());
    let context = RenderContext::for_state(plan.state(), &label);
    let html = render::render(candidate, &context, appendix).html;
    Ok(with_comment_affordance(&html, &page_version(&bytes)))
}

/// The human label for the render watermark: the state name and, where present,
/// the wave.
fn state_label(state: &RunState) -> String {
    match state {
        RunState::Reviewing { wave, .. } => format!("reviewing · wave {}", wave.get()),
        RunState::Correcting { wave, .. } => format!("correcting · wave {}", wave.get()),
        RunState::AwaitingReviewDecision { wave, .. } => {
            format!("awaiting review decision · wave {}", wave.get())
        }
        RunState::Final { .. } => "final".to_owned(),
        _ => "draft".to_owned(),
    }
}

/// The version stamp embedded in a served page and echoed back on a comment: the
/// digest of the exact plan bytes the page was rendered from.
fn page_version(bytes: &[u8]) -> String {
    Sha256Digest::hash(bytes).as_str().to_owned()
}

/// The comment affordance's styles — a "comment" button that reveals a textarea,
/// keyed to the page's own `--paper`/`--line`/`--accent`/`--muted` tokens. Ported
/// from `render.py`'s `SERVE_CSS`, anchored on `[data-block]` rather than lines.
const SERVE_CSS: &str = "\
.revy-add { max-width: 54rem; margin: .1rem auto .6rem; display: flex; gap: .5rem; align-items: flex-start; }
.revy-add button { font: inherit; font-size: .75rem; color: var(--muted); cursor: pointer; background: var(--paper); border: 1px solid var(--line); border-radius: 5px; padding: .1rem .5rem; opacity: .55; transition: opacity .12s; }
[data-block]:hover + .revy-add button, .revy-add:hover button, .revy-add:focus-within button, .revy-add.open button { opacity: 1; }
.revy-add textarea { flex: 1; font: inherit; font-size: .9rem; color: var(--ink); background: var(--paper); border: 1px solid var(--accent); border-radius: 6px; padding: .4rem .6rem; resize: vertical; }
.revy-err { color: var(--error); font-size: .8rem; }";

/// The comment affordance's script: a "comment" button after every `[data-block]`
/// that posts to `/comment` with the block token and the page version stamp, then
/// reloads on success. Ported from `render.py`'s `SERVE_JS`.
const SERVE_JS: &str = r#"
(() => {
  const post = async (block, text) => {
    const r = await fetch("/comment", {
      method: "POST", headers: {"Content-Type": "application/json"},
      body: JSON.stringify({block, text, version: REVY_VERSION}),
    });
    if (r.ok) { location.reload(); return ""; }
    return await r.text();
  };
  for (const el of document.querySelectorAll("[data-block]")) {
    const bar = document.createElement("div");
    bar.className = "revy-add";
    const open = document.createElement("button");
    open.type = "button";
    open.textContent = "comment";
    bar.appendChild(open);
    el.insertAdjacentElement("afterend", bar);
    open.addEventListener("click", () => {
      bar.classList.add("open");
      open.remove();
      const box = document.createElement("textarea");
      box.rows = 3;
      box.placeholder = "a plain comment — it lands as a blockquote on this block";
      const save = document.createElement("button");
      save.type = "button";
      save.textContent = "save";
      const err = document.createElement("span");
      err.className = "revy-err";
      bar.append(box, save, err);
      box.focus();
      const submit = async () => {
        if (!box.value.trim()) return;
        save.disabled = true;
        err.textContent = await post(el.dataset.block, box.value);
        save.disabled = false;
      };
      save.addEventListener("click", submit);
      box.addEventListener("keydown", (e) => {
        if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) submit();
      });
    });
  }
})();
"#;

/// The lifecycle watchdog's script: a heartbeat every few seconds while the tab
/// is open, and a best-effort close signal on unload. `sendBeacon` is used for
/// `/bye` because, unlike `fetch`, it is guaranteed to be sent even as the page
/// is torn down. Injected into every served page (see [`with_watchdog_script`]),
/// including diagnostics pages, so the server closes itself once nothing is
/// looking at it — see [`should_exit`] for the exit decision this feeds.
const WATCHDOG_JS: &str = r#"
(() => {
  setInterval(() => fetch("/heartbeat").catch(() => {}), 3000);
  const bye = () => navigator.sendBeacon("/bye");
  addEventListener("pagehide", bye);
  addEventListener("beforeunload", bye);
})();
"#;

/// Insert `snippet` just before a page's `</body>` (or append it if the page
/// has none), so injected assets land at the end of the document.
fn inject_before_body(html: &str, snippet: &str) -> String {
    html.rfind("</body>").map_or_else(
        || format!("{html}{snippet}"),
        |index| format!("{}{}{}", &html[..index], snippet, &html[index..]),
    )
}

/// Inject the comment affordance into a rendered page, just before `</body>`, so
/// the served page is the static render plus the button and its version stamp.
fn with_comment_affordance(html: &str, version: &str) -> String {
    let version = serde_json::to_string(version).unwrap_or_else(|_| "\"\"".to_owned());
    let assets = format!(
        "<style>{SERVE_CSS}</style><script>const REVY_VERSION={version};{SERVE_JS}</script>"
    );
    inject_before_body(html, &assets)
}

/// Inject the watchdog script into any served page (see [`WATCHDOG_JS`]).
fn with_watchdog_script(html: &str) -> String {
    inject_before_body(html, &format!("<script>{WATCHDOG_JS}</script>"))
}

/// A comment posted from the served page.
#[derive(Debug, serde::Deserialize)]
struct CommentRequest {
    block: String,
    text: String,
    version: String,
}

/// Why a posted comment was refused. The message is returned as the response
/// body and shown inline beside the textarea.
#[derive(Debug, Error)]
enum CommentError {
    #[error("the comment request was malformed")]
    Malformed,
    #[error("the plan changed since this page was rendered — reload, then comment again")]
    Stale,
    #[error("the comment names no block on this page")]
    BadAnchor,
    #[error("a comment cannot be empty")]
    Empty,
    #[error("only a final plan accepts comments")]
    NotFinal,
    #[error("the comment did not produce a valid plan")]
    Rejected,
    #[error("could not read the plan")]
    Read,
    #[error("could not save the comment: {0}")]
    Save(#[source] ArtifactError),
}

impl CommentError {
    const fn status(&self) -> u16 {
        match self {
            Self::Malformed | Self::BadAnchor | Self::Empty => 400,
            Self::Stale => 409,
            Self::NotFinal | Self::Rejected => 422,
            Self::Read | Self::Save(_) => 500,
        }
    }
}

/// Append an open comment to the plan's typed overlay: validate the request
/// against the current file (version stamp, a real block anchor, non-empty text),
/// insert it into `run.comments`, and re-parse before saving so a malformed
/// overlay can never reach disk.
fn handle_comment(path: &Path, body: &[u8]) -> Result<(), CommentError> {
    let request: CommentRequest =
        serde_json::from_slice(body).map_err(|_| CommentError::Malformed)?;
    let bytes = std::fs::read(path).map_err(|_| CommentError::Read)?;
    if request.version != page_version(&bytes) {
        return Err(CommentError::Stale);
    }
    let anchor = BlockAnchor::parse_token(&request.block).ok_or(CommentError::BadAnchor)?;
    let text = NonEmptyText::parse(&request.text).map_err(|_| CommentError::Empty)?;

    let mut document: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| CommentError::Malformed)?;
    let run = document
        .get_mut("run")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or(CommentError::NotFinal)?;
    if run.get("state").and_then(serde_json::Value::as_str) != Some("final") {
        return Err(CommentError::NotFinal);
    }
    // `RawRunState` is adjacently tagged, so the variant body — and its comment
    // overlay — lives under `run.data`.
    let data = run
        .get_mut("data")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or(CommentError::NotFinal)?;
    let entry = serde_json::json!({
        "anchor": {"section": anchor.section, "block": anchor.block},
        "text": text.as_str(),
        "resolved": false,
    });
    data.entry("comments")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .ok_or(CommentError::NotFinal)?
        .push(entry);

    let updated = serde_json::to_vec(&document).map_err(|_| CommentError::Malformed)?;
    let plan = ParsedPlan::parse(&updated).map_err(|_| CommentError::Rejected)?;
    artifact::save_atomic(path, &plan).map_err(CommentError::Save)
}

fn diagnostic_html(code: &str, message: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Revy plan diagnostics</title></head><body><main><h1>Revy plan diagnostics</h1><ul><li><strong>{}</strong>: {}</li></ul></main></body></html>",
        escape_html(code),
        escape_html(message)
    )
}

fn escape_html(raw: &str) -> String {
    raw.chars()
        .flat_map(|character| match character {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect(),
            '>' => "&gt;".chars().collect(),
            '\"' => "&quot;".chars().collect(),
            '\'' => "&#39;".chars().collect(),
            _ => std::iter::once(character).collect(),
        })
        .collect()
}

fn html_response(html: &str) -> Result<Response<std::io::Cursor<Vec<u8>>>, ServeError> {
    let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8")
        .map_err(|()| ServeError::ContentTypeHeader)?;
    let html = with_watchdog_script(html);
    Ok(Response::from_data(html.into_bytes()).with_header(header))
}

/// A plain-text response the comment affordance shows inline (the error message).
fn text_response(status: u16, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(message.as_bytes().to_vec()).with_status_code(StatusCode(status))
}

/// The lock sibling of a plan file: `<plan>.lock` (the same file `apply`/`format`
/// take an exclusive lock on).
fn lock_path(plan_path: &Path) -> PathBuf {
    let mut name = plan_path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// Read the plan under a shared advisory lock on `<plan>.lock`, taken per request
/// rather than for the whole serve lifetime (design §10: status/render/serve take
/// a shared lock for the read). A concurrent `apply`/`format` holds the exclusive
/// lock, so the page is rendered from a plan file no writer is mid-replacing.
fn with_shared_read<T>(plan_path: &Path, body: impl FnOnce() -> T) -> Result<T, std::io::Error> {
    let lock = lock_path(plan_path);
    if let Some(parent) = lock.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock)?;
    FileExt::lock_shared(&file)?;
    let result = body();
    let _ = FileExt::unlock(&file);
    Ok(result)
}

/// Route one request: the page at `GET /`, a comment at `POST /comment`, the
/// watchdog's heartbeat at `GET /heartbeat` and close signal at `POST /bye`,
/// 404 otherwise. `POST` reads the body, so the request is borrowed mutably
/// here and responded to by the caller.
fn respond(
    request: &mut tiny_http::Request,
    plan_path: &Path,
    cache: &mut Cache,
    watchdog: &mut Watchdog,
) -> Result<Response<std::io::Cursor<Vec<u8>>>, ServeError> {
    match (request.method(), request.url()) {
        (Method::Get, "/") => {
            let html =
                with_shared_read(plan_path, || cache.response(plan_path)).unwrap_or_else(|error| {
                    diagnostic_html("plan-lock-unavailable", &error.to_string())
                });
            html_response(&html)
        }
        (Method::Get, "/heartbeat") => {
            watchdog.last_heartbeat = Some(Instant::now());
            Ok(Response::from_data(Vec::new()).with_status_code(StatusCode(204)))
        }
        (Method::Post, "/bye") => {
            watchdog.bye_at = Some(Instant::now());
            Ok(Response::from_data(Vec::new()).with_status_code(StatusCode(204)))
        }
        (Method::Post, "/comment") => {
            let mut body = String::new();
            if request.as_reader().read_to_string(&mut body).is_err() {
                return Ok(text_response(400, "could not read the comment"));
            }
            Ok(match handle_comment(plan_path, body.as_bytes()) {
                Ok(()) => Response::from_data(Vec::new()),
                Err(error) => text_response(error.status(), &error.to_string()),
            })
        }
        _ => Ok(Response::from_data(Vec::new()).with_status_code(StatusCode(404))),
    }
}

/// Bind the requested port on the loopback interface. An already-taken port is a
/// hard failure that names it: the caller asked for this exact port, so honour
/// that (fail loud) rather than silently serving on a different one the user
/// never sees. Port leaks are handled by the idle self-shutdown below, not by
/// hunting for a spare port here.
fn bind(port: u16) -> Result<Server, ServeError> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|source| ServeError::Bind {
        port,
        source: Box::new(source),
    })?;
    Server::from_listener(listener, None).map_err(|source| ServeError::Bind { port, source })
}

/// How long to wait, once heartbeats have arrived at least once, for the next
/// one before concluding the tab closed or crashed without sending `/bye` — a
/// few missed 3s heartbeat intervals.
const IDLE_TIMEOUT: Duration = Duration::from_secs(12);
/// How long to wait after a `/bye` signal before actually exiting — long
/// enough to absorb a page reload, whose unload fires `/bye` just before the
/// reloaded page's first heartbeat cancels the shutdown.
const BYE_GRACE: Duration = Duration::from_secs(3);
/// How often the accept loop wakes with no request pending, so the watchdog
/// checks above run promptly instead of only between real requests.
const WATCHDOG_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// The lifecycle watchdog's observed state: the last heartbeat and the last
/// `/bye` signal, if either has happened yet.
#[derive(Debug, Clone, Copy, Default)]
struct Watchdog {
    last_heartbeat: Option<Instant>,
    bye_at: Option<Instant>,
}

/// Decide whether the serve loop should exit now. The server auto-closes only
/// after a tab has actually connected and then gone away — never while nobody has
/// opened the page yet, so a served URL waits indefinitely for its first visit.
/// Two reasons, checked in order:
/// - an explicit `/bye` past [`BYE_GRACE`] and not cancelled by a later
///   heartbeat (a reload sends `/bye` on unload, then resumes heartbeats);
/// - a heartbeat has arrived before but none for [`IDLE_TIMEOUT`] (the tab
///   closed or crashed without a clean `/bye`).
fn should_exit(watchdog: Watchdog, now: Instant) -> bool {
    if let Some(bye_at) = watchdog.bye_at {
        let cancelled = watchdog.last_heartbeat.is_some_and(|beat| beat > bye_at);
        if !cancelled && now.saturating_duration_since(bye_at) >= BYE_GRACE {
            return true;
        }
    }
    watchdog
        .last_heartbeat
        .is_some_and(|beat| now.saturating_duration_since(beat) >= IDLE_TIMEOUT)
}

pub fn run(plan_path: &Path, port: u16, appendix: Appendix) -> Result<(), ServeError> {
    let server = bind(port)?;
    let actual_port = server
        .server_addr()
        .to_ip()
        .map_or(port, |addr| addr.port());
    println!("http://127.0.0.1:{actual_port}/");
    std::io::stdout().flush().map_err(ServeError::Print)?;

    let mut cache = Cache::new(appendix);
    let mut watchdog = Watchdog::default();

    loop {
        if should_exit(watchdog, Instant::now()) {
            eprintln!("revy plan serve: browser tab closed — exiting");
            break;
        }
        let mut request = match server.recv_timeout(WATCHDOG_POLL_INTERVAL) {
            Ok(Some(request)) => request,
            Ok(None) => continue,
            Err(source) => return Err(ServeError::Accept(source)),
        };
        let response = respond(&mut request, plan_path, &mut cache, &mut watchdog)?;
        let _ = request.respond(response);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
    use super::*;

    const FINAL_PLAN: &[u8] = include_bytes!("../tests/fixtures/final-plan.json");

    /// A throwaway plan file seeded with the final-plan fixture, removed on drop.
    struct TempPlan(PathBuf);

    impl TempPlan {
        fn new(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("revy-serve-{tag}-{nanos}.json"));
            std::fs::write(&path, FINAL_PLAN).unwrap();
            Self(path)
        }

        fn version(&self) -> String {
            page_version(&std::fs::read(&self.0).unwrap())
        }
    }

    impl Drop for TempPlan {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn body(block: &str, text: &str, version: &str) -> String {
        serde_json::json!({"block": block, "text": text, "version": version}).to_string()
    }

    #[test]
    fn test_a_posted_comment_lands_as_an_open_comment_on_the_plan() {
        let plan = TempPlan::new("post");
        let request = body("s0b0", "this path double-locks", &plan.version());

        handle_comment(&plan.0, request.as_bytes()).expect("the comment is accepted");

        let saved = artifact::load(&plan.0).expect("the plan still parses");
        let comments = saved.state().comments();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].anchor, BlockAnchor::new(0, 0));
        assert!(!comments[0].resolved);
    }

    #[test]
    fn test_a_stale_version_is_refused_so_the_plan_is_untouched() {
        let plan = TempPlan::new("stale");
        let request = body("s0b0", "late", "not-the-current-digest");

        let error = handle_comment(&plan.0, request.as_bytes()).expect_err("stale is refused");

        assert!(matches!(error, CommentError::Stale));
        assert_eq!(artifact::load(&plan.0).unwrap().state().comments(), []);
    }

    #[test]
    fn test_a_malformed_block_anchor_is_refused() {
        let plan = TempPlan::new("anchor");
        let request = body("not-a-token", "hi", &plan.version());

        let error = handle_comment(&plan.0, request.as_bytes()).expect_err("bad anchor is refused");

        assert!(matches!(error, CommentError::BadAnchor));
    }

    #[test]
    fn test_an_empty_comment_is_refused() {
        let plan = TempPlan::new("empty");
        let request = body("s0b0", "   ", &plan.version());

        let error = handle_comment(&plan.0, request.as_bytes()).expect_err("empty is refused");

        assert!(matches!(error, CommentError::Empty));
    }

    #[test]
    fn test_a_brief_cache_serves_the_page_without_the_appendix() {
        let plan = TempPlan::new("brief");
        let mut cache = Cache::new(Appendix::Omitted);

        let html = cache.response(&plan.0);

        assert!(html.contains("<h2>Delivery</h2>"), "{html}");
        assert!(
            !html.contains("<details class=\"section secondary appendix\">"),
            "{html}"
        );
    }

    #[test]
    fn test_a_full_cache_serves_the_page_with_the_appendix() {
        let plan = TempPlan::new("full");
        let mut cache = Cache::new(Appendix::Included);

        let html = cache.response(&plan.0);

        assert!(
            html.contains("<details class=\"section secondary appendix\">"),
            "{html}"
        );
    }

    #[test]
    fn test_bind_uses_the_preferred_port_when_it_is_free() {
        let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let preferred = probe.local_addr().unwrap().port();
        drop(probe);

        let server = bind(preferred).expect("binds the preferred, now-free port");

        assert_eq!(server.server_addr().to_ip().unwrap().port(), preferred);
    }

    #[test]
    fn test_bind_fails_loud_when_the_requested_port_is_taken() {
        let held = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let requested = held.local_addr().unwrap().port();

        // The caller asked for this exact port; a conflict is a hard, named
        // failure rather than a silent bind on some other port. (`Server` has no
        // `Debug`, so match rather than `expect_err`.)
        let result = bind(requested);

        assert!(
            matches!(result, Err(ServeError::Bind { port, .. }) if port == requested),
            "a taken port must fail loud with a named bind error"
        );
    }

    #[test]
    fn test_a_server_nobody_has_opened_never_exits() {
        // No heartbeat has ever arrived, so no tab has connected: the server waits
        // indefinitely for its first visit however long it has been up.
        let now = Instant::now() + Duration::from_hours(1);

        assert!(!should_exit(Watchdog::default(), now));
    }

    #[test]
    fn test_a_recent_heartbeat_within_the_idle_timeout_does_not_exit() {
        let started_at = Instant::now();
        let beat = started_at + Duration::from_secs(100);
        let watchdog = Watchdog {
            last_heartbeat: Some(beat),
            bye_at: None,
        };
        let now = (beat + IDLE_TIMEOUT)
            .checked_sub(Duration::from_secs(1))
            .unwrap();

        assert!(!should_exit(watchdog, now));
    }

    #[test]
    fn test_a_heartbeat_past_the_idle_timeout_exits() {
        let started_at = Instant::now();
        let beat = started_at + Duration::from_secs(100);
        let watchdog = Watchdog {
            last_heartbeat: Some(beat),
            bye_at: None,
        };
        let now = beat + IDLE_TIMEOUT;

        assert!(should_exit(watchdog, now));
    }

    #[test]
    fn test_bye_within_its_grace_window_does_not_exit_yet() {
        let started_at = Instant::now();
        let beat = started_at + Duration::from_secs(5);
        let bye_at = beat + Duration::from_secs(1);
        let watchdog = Watchdog {
            last_heartbeat: Some(beat),
            bye_at: Some(bye_at),
        };
        let now = (bye_at + BYE_GRACE)
            .checked_sub(Duration::from_secs(1))
            .unwrap();

        assert!(!should_exit(watchdog, now));
    }

    #[test]
    fn test_bye_past_its_grace_window_with_no_later_heartbeat_exits() {
        let started_at = Instant::now();
        let beat = started_at + Duration::from_secs(5);
        let bye_at = beat + Duration::from_secs(1);
        let watchdog = Watchdog {
            last_heartbeat: Some(beat),
            bye_at: Some(bye_at),
        };
        let now = bye_at + BYE_GRACE;

        assert!(should_exit(watchdog, now));
    }

    #[test]
    fn test_a_heartbeat_after_bye_cancels_the_shutdown() {
        let started_at = Instant::now();
        let bye_at = started_at + Duration::from_secs(5);
        let reload_beat = bye_at + Duration::from_secs(1);
        let watchdog = Watchdog {
            last_heartbeat: Some(reload_beat),
            bye_at: Some(bye_at),
        };
        let now = bye_at + BYE_GRACE + Duration::from_secs(1);

        assert!(!should_exit(watchdog, now));
    }
}
