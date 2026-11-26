//! Runtime-evidence transcripts resolve against the plan's own `transcripts/`
//! first, then the shared content-addressed store by digest.
#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use common::{DIGEST, REVISION, document, parse, report, unmoved_checkouts};
use revy_plan::document::ParsedPlan;
use revy_plan::packet::WorkKind;
use revy_plan::primitives::Sha256Digest;
use revy_plan::runtime::{CheckError, TranscriptStores};
use revy_plan::workflow::frontier;
use serde_json::{Value, json};
use tempfile::TempDir;

fn exploring() -> ParsedPlan {
    parse(&document(
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": [] },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        }),
    ))
}

fn runtime_result(digest: &Sha256Digest) -> Value {
    json!({
        "evidence": [{
            "kind": "runtime", "id": "evidence-run",
            "command": { "kind": "program", "program": "cargo", "args": ["test"] },
            "target": { "revision": REVISION, "dirty_digest": DIGEST }, "observation": "Passed.",
            "artifact": { "path": "run.txt", "digest": digest.as_str() }, "limitation": null
        }],
        "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "propose_capability", "report": report() }
    })
}

struct Stores {
    plan: TempDir,
    shared: TempDir,
}

impl Stores {
    fn new() -> Self {
        Self {
            plan: tempfile::tempdir().expect("plan transcripts"),
            shared: tempfile::tempdir().expect("shared store"),
        }
    }

    fn check(&self, plan: &ParsedPlan, result: &Value) -> Result<(), Vec<CheckError>> {
        let frontier = frontier(plan);
        let packet = frontier
            .iter()
            .find(|packet| packet.kind() == WorkKind::Exploration)
            .expect("exploration packet");
        let raw = packet
            .decode_result(result.to_string().as_bytes())
            .expect("decodes");
        let stores = TranscriptStores {
            plan: self.plan.path(),
            shared: self.shared.path(),
        };
        revy_plan::runtime::check_result(plan, stores, &unmoved_checkouts(), raw)
            .map(|_checked| ())
            .map_err(|rejection| {
                rejection
                    .into_parts()
                    .0
                    .into_vec()
                    .into_iter()
                    .map(revy_plan::issues::Located::into_error)
                    .collect()
            })
    }
}

#[test]
fn a_transcript_found_only_in_the_shared_store_verifies_by_digest() {
    let stores = Stores::new();
    let digest = Sha256Digest::hash(b"ok\n");
    std::fs::write(stores.shared.path().join(digest.as_str()), "ok\n").unwrap();

    let outcome = stores.check(&exploring(), &runtime_result(&digest));

    assert_eq!(outcome, Ok(()));
}

#[test]
fn a_mismatched_plan_local_transcript_falls_back_to_the_shared_store() {
    let stores = Stores::new();
    let digest = Sha256Digest::hash(b"ok\n");
    std::fs::write(stores.plan.path().join("run.txt"), "tampered\n").unwrap();
    std::fs::write(stores.shared.path().join(digest.as_str()), "ok\n").unwrap();

    let outcome = stores.check(&exploring(), &runtime_result(&digest));

    assert_eq!(outcome, Ok(()));
}

#[test]
fn a_mismatched_plan_local_transcript_without_a_shared_copy_names_both_lookups() {
    let stores = Stores::new();
    let digest = Sha256Digest::hash(b"ok\n");
    std::fs::write(stores.plan.path().join("run.txt"), "tampered\n").unwrap();

    let outcome = stores.check(&exploring(), &runtime_result(&digest));

    let error = outcome.unwrap_err().remove(0).to_string();
    assert_eq!(
        error,
        format!(
            "evidence `evidence-run` artifact `run.txt` does not verify against its declared digest `{}`: the plan's copy `{}` hashes to `{}`; the shared store `{}` has no `{}`; run `revy-plan transcript add FILE` and use the reference it prints",
            digest.as_str(),
            stores.plan.path().join("run.txt").display(),
            Sha256Digest::hash(b"tampered\n").as_str(),
            stores.shared.path().display(),
            digest.as_str(),
        )
    );
}

#[test]
fn a_transcript_in_neither_store_names_both_locations() {
    let stores = Stores::new();
    let digest = Sha256Digest::hash(b"ok\n");

    let outcome = stores.check(&exploring(), &runtime_result(&digest));

    let error = outcome.unwrap_err().remove(0).to_string();
    assert_eq!(
        error,
        format!(
            "evidence `evidence-run` artifact `run.txt` (digest `{}`) is in neither the plan's transcripts `{}` nor the shared store `{}`; run `revy-plan transcript add FILE` and use the reference it prints",
            digest.as_str(),
            stores.plan.path().display(),
            stores.shared.path().display()
        )
    );
}
