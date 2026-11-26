//! Choosing the ready packet a result answers: the sole packet, a work key, or an
//! unambiguous kind name.
#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use common::{concurrency_candidate, minimal_plan_document, parse, reviewing_document};
use revy_plan::document::ParsedPlan;
use revy_plan::packet::{SelectError, WorkSelector};
use revy_plan::primitives::NonEmptyText;
use revy_plan::workflow::frontier;
use serde_json::json;

fn named(raw: &str) -> WorkSelector {
    WorkSelector::Named(NonEmptyText::parse(raw).unwrap())
}

fn drafting() -> ParsedPlan {
    parse(&common::drafting_document(&common::simple_candidate()))
}

fn two_specialists() -> ParsedPlan {
    let mut candidate = concurrency_candidate();
    candidate["content"]["risks"] = json!([{
        "kind": "specialist", "id": "risk-1", "focus": "security",
        "statement": "Adds a new trust boundary.", "mitigation": "Threat-model it."
    }]);
    parse(&reviewing_document(
        candidate,
        json!([
            { "kind": "integrated" },
            { "kind": "specialist", "focus": "concurrency" },
            { "kind": "specialist", "focus": "security" }
        ]),
    ))
}

#[test]
fn no_selector_picks_the_sole_ready_packet() {
    let plan = drafting();
    let frontier = frontier(&plan);

    let packet = frontier.select(&WorkSelector::Sole).expect("one packet");

    assert_eq!(packet.kind().name(), "draft");
}

#[test]
fn no_selector_over_several_ready_packets_lists_them() {
    let plan = parse(&minimal_plan_document());
    let frontier = frontier(&plan);

    let error = frontier
        .select(&WorkSelector::Sole)
        .expect_err("two packets");

    assert!(matches!(error, SelectError::SeveralReady { ref offered } if offered.len() == 2));
}

#[test]
fn a_kind_name_picks_the_only_packet_of_that_kind() {
    let plan = parse(&minimal_plan_document());
    let frontier = frontier(&plan);

    let packet = frontier
        .select(&named("capability_approval"))
        .expect("one approval packet");

    assert_eq!(packet.kind().name(), "capability_approval");
}

#[test]
fn a_work_key_picks_its_packet() {
    let plan = drafting();
    let frontier = frontier(&plan);
    let key = frontier.only().unwrap().key().as_str().to_owned();

    let packet = frontier.select(&named(&key)).expect("the keyed packet");

    assert_eq!(packet.key().as_str(), key);
}

#[test]
fn a_kind_name_shared_by_several_packets_is_ambiguous() {
    let plan = two_specialists();
    let frontier = frontier(&plan);

    let error = frontier
        .select(&named("specialist_review"))
        .expect_err("two specialists");

    assert!(matches!(error, SelectError::AmbiguousKind { ref keys, .. } if keys.len() == 2));
}

#[test]
fn an_unknown_selector_is_rejected() {
    let plan = drafting();
    let frontier = frontier(&plan);

    let error = frontier
        .select(&named("exploration"))
        .expect_err("not ready");

    assert!(matches!(error, SelectError::Unknown { .. }));
}

#[test]
fn a_final_plan_offers_nothing_to_select() {
    let plan = parse(&common::final_plan_document());
    let frontier = frontier(&plan);

    let error = frontier.select(&WorkSelector::Sole).expect_err("terminal");

    assert_eq!(error, SelectError::Terminal);
}
