# revy-plan

Portable bootstrap skill for the Rust `revy-plan` controller.

## Prerequisites

- A `revy-plan` binary on `PATH`.
- Filesystem and shell access for the target checkout and the external plan home, where `plan.json` lives.
- The evidence capabilities available now, named by purpose not product — code-intelligence, runtime execution, structured and text search. No tool is mandatory; reading source and grep are the universal floor.
- An isolated review context so each review runs apart from the draft it reads.
- A delivery path: a reachable loopback URL, or the standalone HTML artifact.

Visual-native tools can improve review, but they are optional. A headless harness can still produce and deliver `plan.html`.

## Binary lookup

The skill assumes `revy-plan` is on `PATH` — install it with `cargo install --path cli` from the package root. `check_docs.py` resolves the binary the same way: `$REVY_BIN` when set, then `revy-plan` on `PATH`, then a `cargo run` fallback against `cli/Cargo.toml`.

Inventory the capabilities into `capabilities.json`, then create the plan:

```bash
revy-plan init --capabilities capabilities.json
```

Verify the controller before invoking the skill:

```bash
revy-plan status --help
```

The skill body is intentionally small. Runtime workflow, schemas, packet mandates, validation, and rendering come from the current `revy-plan status --format json` output. The plan finishes when `status` prints `final`; `revy-plan render` and `revy-plan serve` may run at any candidate-bearing state and carry a draft watermark until then.
