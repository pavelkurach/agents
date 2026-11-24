#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Validate revy-plan docs against the real CLI surfaces they point at."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent
PKG = ROOT.parent
README = ROOT / "README.md"
SKILL = ROOT / "SKILL.md"
DOCS = [SKILL, README, *sorted((ROOT / "references").glob("*.md"))]
MINIMAL = PKG / "cli/tests/fixtures/evidence-first-minimal-plan.json"
COMPLETE = PKG / "cli/tests/fixtures/final-plan.json"
LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
REF_USAGE_RE = re.compile(r"\[([^\]]+)\]\[([^\]]*)\]")
REF_LINK_RE = re.compile(r"^\[([^\]]+)\]:\s*(\S+)", re.M)
SHORTCUT_REF_RE = re.compile(r"(?<!!)\[([^\]]+)\](?!\(|\[|:)")

problems: list[str] = []


def fail(path: Path, message: str) -> None:
    problems.append(f"{path.resolve().relative_to(ROOT.resolve())}: {message}")


def command() -> list[str] | None:
    if env := os.environ.get("REVY_BIN"):
        return [env]
    if path := shutil.which("revy-plan"):
        return [path]
    manifest = PKG / "cli/Cargo.toml"
    if manifest.exists():
        return [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(manifest),
            "--bin",
            "revy-plan",
            "--",
        ]
    return None


def run(args: list[str]) -> subprocess.CompletedProcess[str]:
    cmd = command()
    if cmd is None:
        raise RuntimeError("no revy-plan binary and no Rust CLI checkout")
    return subprocess.run([*cmd, *args], cwd=PKG, capture_output=True, text=True)


def status(path: Path) -> dict | None:
    out = run(["status", "--format", "json", "--plan", str(path)])
    if out.returncode != 0:
        fail(SKILL, f"`revy-plan status` failed for {path.name}: {out.stderr.strip()}")
        return None
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError as exc:
        fail(SKILL, f"`revy-plan status` emitted non-JSON for {path.name}: {exc}")
        return None


def local_links(path: Path) -> list[Path]:
    body = path.read_text(encoding="utf-8")
    references = {
        label.lower(): target
        for label, target in REF_LINK_RE.findall(body)
    }
    root = ROOT.resolve()
    out: list[Path] = []

    def add_target(target: str) -> None:
        target = target.split("#", 1)[0]
        if not target or "://" in target or not target.endswith(".md"):
            return
        resolved = (path.parent / target).resolve()
        if not resolved.is_relative_to(root):
            fail(path, f"local link escapes skill directory: {target}")
            return
        if not resolved.exists():
            fail(path, f"unresolved local link: {target}")
            return
        out.append(resolved)

    for match in LINK_RE.finditer(body):
        raw = match.group(1)
        add_target(references.get(raw.lower(), raw))
    for match in REF_USAGE_RE.finditer(body):
        label = (match.group(2) or match.group(1)).lower()
        if label not in references:
            fail(path, f"unresolved local reference link: {label}")
            continue
        add_target(references[label])
    for match in SHORTCUT_REF_RE.finditer(body):
        label = match.group(1).lower()
        if label not in references:
            continue
        add_target(references[label])
    return out


def reachable_local_markdown(start: Path) -> list[Path]:
    seen: set[Path] = set()
    ordered: list[Path] = []
    todo = [start.resolve()]
    while todo:
        path = todo.pop(0)
        if path in seen:
            continue
        seen.add(path)
        ordered.append(path)
        todo.extend(local_links(path))
    return ordered


def check_links() -> None:
    reachable_local_markdown(SKILL)


def check_help_surfaces() -> None:
    readme = README.read_text(encoding="utf-8")
    if "revy-plan status --help" not in readme:
        fail(README, "binary lookup no longer names `revy-plan status --help`")
    for args, needle in (
        (["status", "--help"], "--format"),
        (["serve", "--help"], "--plan"),
        (["render", "--help"], "--out"),
    ):
        out = run(args)
        surface = " ".join(args)
        if out.returncode != 0:
            fail(README, f"`revy-plan {surface}` failed: {out.stderr.strip()}")
        elif needle not in out.stdout:
            fail(README, f"`revy-plan {surface}` help omits {needle!r}")


def check_ready_packet(packet: dict, expected_kind: str) -> None:
    if packet.get("kind") != expected_kind:
        fail(SKILL, f"expected {expected_kind} packet, got {packet.get('kind')!r}")
    for field in ("key", "kind", "subject", "mandate", "context", "schema_ref"):
        if field not in packet:
            fail(SKILL, f"{expected_kind} packet omits {field}")
    subject = packet.get("subject")
    if not isinstance(subject, dict) or "subject" not in subject:
        fail(SKILL, f"{expected_kind} packet subject is not typed")


def check_status_packets() -> None:
    minimal = status(MINIMAL)
    if minimal is not None:
        packets = minimal.get("packets") if isinstance(minimal, dict) else None
        if not isinstance(packets, list) or not packets:
            fail(SKILL, "minimal fixture status is not a state object with a non-empty packets array")
        else:
            check_ready_packet(packets[0], "capability_approval")

    final = status(COMPLETE)
    if final is not None:
        if not isinstance(final, dict):
            fail(SKILL, "final fixture status is not a state object")
        else:
            if final.get("state") != "final":
                fail(SKILL, f"final fixture status is {final.get('state')!r}, expected state 'final'")
            if final.get("packets") != []:
                fail(SKILL, "final fixture status still emits packets")


def main() -> int:
    try:
        check_links()
        check_help_surfaces()
        check_status_packets()
    except RuntimeError as exc:
        fail(README, str(exc))

    if problems:
        print("check_docs.py")
        for problem in problems:
            print(f"  FAIL  {problem}")
        print(f"\n{len(problems)} failed")
        return 1

    print("check_docs.py")
    print("  PASS  docs match CLI status/serve/render surfaces")
    return 0


if __name__ == "__main__":
    sys.exit(main())
