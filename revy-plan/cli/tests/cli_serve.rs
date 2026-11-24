//! The stable loopback server renders any candidate-bearing plan (design §9, §11).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

use assert_cmd::cargo::cargo_bin;
use tempfile::TempDir;

const FINAL_PLAN: &str = include_str!("fixtures/final-plan.json");
const MINIMAL_PLAN: &str = include_str!("fixtures/evidence-first-minimal-plan.json");

struct ServeProcess {
    child: Child,
    stdout: BufReader<ChildStdout>,
}

impl ServeProcess {
    fn stop(mut self) {
        self.child.kill().expect("kill serve process");
        self.child.wait().expect("wait for serve process");
    }
}

impl Drop for ServeProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn plan_with(dir: &TempDir, body: &str) -> PathBuf {
    let plan = dir.path().join("plan.json");
    std::fs::write(&plan, body).expect("write plan");
    plan
}

fn unused_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve a loopback port");
    listener.local_addr().expect("listener address").port()
}

const BIND_ATTEMPTS: usize = 8;

fn serve(plan: &Path) -> (ServeProcess, String) {
    for _ in 0..BIND_ATTEMPTS {
        let port = unused_loopback_port();
        let mut process = spawn_serve(plan, port);
        let mut line = String::new();
        process.stdout.read_line(&mut line).expect("read serve URL");
        if line.trim() == format!("http://127.0.0.1:{port}/") {
            return (process, line);
        }
        assert!(
            line.is_empty(),
            "serve printed an unexpected line: {line:?}"
        );
    }
    panic!("serve could not bind a free loopback port in {BIND_ATTEMPTS} attempts");
}

fn spawn_serve(plan: &Path, port: u16) -> ServeProcess {
    let mut child = Command::new(cargo_bin("revy-plan"))
        .args(["serve", "--plan"])
        .arg(plan)
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let stdout = child.stdout.take().expect("serve stdout is piped");
    ServeProcess {
        child,
        stdout: BufReader::new(stdout),
    }
}

fn http_get(url: &str) -> (u16, String, String) {
    let address = url
        .strip_prefix("http://")
        .and_then(|value| value.strip_suffix('/'))
        .expect("loopback URL");
    let mut stream = TcpStream::connect(address).expect("connect to serve");
    stream
        .write_all(
            format!("GET / HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .expect("write request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    let (head, body) = response.split_once("\r\n\r\n").expect("HTTP response body");
    let status = head
        .split_whitespace()
        .nth(1)
        .expect("HTTP status")
        .parse()
        .expect("numeric HTTP status");
    (status, head.to_owned(), body.to_owned())
}

#[test]
fn serve_prints_one_stable_url_and_serves_the_final_plan() {
    let directory = tempfile::tempdir().expect("tempdir");
    let plan = plan_with(&directory, FINAL_PLAN);
    let (process, url) = serve(&plan);
    assert_eq!(url.trim(), format!("http://127.0.0.1:{}/", port_of(&url)));

    let (status, headers, body) = http_get(url.trim());
    assert_eq!(status, 200);
    assert!(headers.contains("Content-Type: text/html; charset=utf-8"));
    assert!(body.contains("Add a bounded queue"));

    process.stop();
    drop(directory);
}

#[test]
fn serve_reports_a_plan_with_no_candidate() {
    let directory = tempfile::tempdir().expect("tempdir");
    let plan = plan_with(&directory, MINIMAL_PLAN);
    let (process, url) = serve(&plan);
    assert!(http_get(url.trim()).2.contains("plan-no-candidate"));
    process.stop();
    drop(directory);
}

#[test]
fn serve_re_renders_the_current_plan_after_an_edit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let plan = plan_with(&directory, FINAL_PLAN);
    let (process, url) = serve(&plan);
    assert!(http_get(url.trim()).2.contains("Add a bounded queue"));

    let edited = std::fs::read_to_string(&plan)
        .expect("read plan")
        .replace("Add a bounded queue", "Add an unbounded queue");
    std::fs::write(&plan, edited).expect("write edited plan");

    assert!(
        http_get(url.trim()).2.contains("Add an unbounded queue"),
        "serve re-renders the edit"
    );
    process.stop();
    drop(directory);
}

#[test]
fn serve_reports_a_broken_plan_instead_of_exiting() {
    let directory = tempfile::tempdir().expect("tempdir");
    let plan = plan_with(&directory, "{ not json");
    let (process, url) = serve(&plan);
    assert!(http_get(url.trim()).2.contains("structure-invalid"));
    process.stop();
    drop(directory);
}

#[test]
fn serve_reports_a_loopback_bind_failure_with_a_usage_exit_code() {
    let directory = tempfile::tempdir().expect("tempdir");
    let plan = plan_with(&directory, FINAL_PLAN);
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve test port");
    let port = listener.local_addr().expect("listener address").port();

    let output = Command::new(cargo_bin("revy-plan"))
        .args(["serve", "--plan"])
        .arg(&plan)
        .arg("--port")
        .arg(port.to_string())
        .output()
        .expect("run serve");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("could not bind"));
    drop(directory);
}

fn port_of(url: &str) -> u16 {
    url.trim()
        .strip_prefix("http://127.0.0.1:")
        .and_then(|rest| rest.strip_suffix('/'))
        .expect("a loopback URL")
        .parse()
        .expect("a numeric port")
}
