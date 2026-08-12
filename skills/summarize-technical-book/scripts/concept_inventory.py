#!/usr/bin/env python3
"""Validate and collate per-chapter concept evidence."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Sequence


class InventoryError(RuntimeError):
    pass


def schema() -> str:
    return json.dumps(
        {
            "concepts": [
                {
                    "name": "Transferable field term",
                    "role": "introduced | elaborated",
                    "brief_take": "This chapter's contribution in one sentence.",
                }
            ]
        },
        indent=2,
    )


def _string(entry: object, field: str, source: Path, index: int) -> str:
    if not isinstance(entry, dict) or not isinstance(entry.get(field), str) or not entry[field].strip():
        raise InventoryError(
            f"{source.name}: concepts[{index}].{field} must be a non-empty string"
        )
    return entry[field].strip()


def _read(path: Path) -> list[dict[str, str]]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise InventoryError(f"cannot read {path}: {error}") from error
    if not isinstance(payload, dict) or not isinstance(payload.get("concepts"), list):
        raise InventoryError(f"{path.name}: root must contain a concepts array")

    entries: list[dict[str, str]] = []
    for index, raw_entry in enumerate(payload["concepts"]):
        name = _string(raw_entry, "name", path, index)
        role = _string(raw_entry, "role", path, index)
        brief_take = _string(raw_entry, "brief_take", path, index)
        if role not in {"introduced", "elaborated"}:
            raise InventoryError(
                f"{path.name}: concepts[{index}].role must be 'introduced' or 'elaborated'"
            )
        entries.append({"name": name, "role": role, "brief_take": brief_take})
    return entries


def _normalize(name: str) -> str:
    return " ".join(name.casefold().split())


def collate_directory(inventory_dir: Path) -> dict[str, list[dict[str, object]]]:
    paths = sorted(inventory_dir.glob("ch*.json"))
    if not paths:
        raise InventoryError(f"no ch*.json inventories found in {inventory_dir}")

    groups: dict[str, dict[str, object]] = {}
    for path in paths:
        for entry in _read(path):
            key = _normalize(entry["name"])
            group = groups.setdefault(key, {"name": entry["name"], "occurrences": []})
            occurrences = group["occurrences"]
            assert isinstance(occurrences, list)
            occurrences.append(
                {
                    "chapter": path.stem,
                    "role": entry["role"],
                    "brief_take": entry["brief_take"],
                }
            )
    concepts = sorted(groups.values(), key=lambda item: str(item["name"]).casefold())
    return {"concepts": concepts}


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("schema")
    collate = subparsers.add_parser("collate")
    collate.add_argument("inventory_dir", type=Path)
    collate.add_argument("output", type=Path)
    args = parser.parse_args(argv)

    if args.command == "schema":
        print(schema())
        return 0
    try:
        result = collate_directory(args.inventory_dir)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    except InventoryError as error:
        parser.exit(1, f"concept_inventory: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
