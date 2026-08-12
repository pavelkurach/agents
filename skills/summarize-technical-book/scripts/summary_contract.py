#!/usr/bin/env python3
"""Print, validate, and assemble the technical-book summary contract."""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
from pathlib import Path
from typing import Sequence


REQUIRED_SECTIONS = ("Summary", "Learnings", "Critical Commentary")
OPTIONAL_SECTIONS = ("Key Concepts", "Technologies & APIs")
VALID_SECTION_ORDERS = frozenset(
    {
        REQUIRED_SECTIONS,
        ("Summary", "Learnings", "Key Concepts", "Critical Commentary"),
        ("Summary", "Learnings", "Technologies & APIs", "Critical Commentary"),
        (
            "Summary",
            "Learnings",
            "Key Concepts",
            "Technologies & APIs",
            "Critical Commentary",
        ),
    }
)
CHAPTER_HEADING = re.compile(r"^## ((?:Chapter|Chapters|Section|Appendix)\b.+)$", re.MULTILINE)
H2_HEADING = re.compile(r"^## (.+)$", re.MULTILINE)
H3_HEADING = re.compile(r"^### (.+)$", re.MULTILINE)
LEARNING_BULLET = re.compile(r"^- \*\*[^*]+\*\*(?:\s|$)", re.MULTILINE)


class ContractError(RuntimeError):
    pass


def schema() -> str:
    return """\
## Chapter N: Title

### Summary
Concise argument-and-evidence context for the learnings.

### Learnings
- **Decision-changing takeaway.** Mechanism, applicability, trade-off, or consequence.

### Key Concepts
- **Transferable field term**: Definition that explains or predicts behavior.
[Omit this heading when the chapter introduces or elaborates no transferable concepts.]

### Technologies & APIs
- **Concrete technology or API family**: When it is useful and why it matters.
[Omit this heading when the chapter has no applicable entries.]

### Critical Commentary
Corrections and external additions with provenance labels.

## Book-wide Learnings
- **Deduplicated engineering lesson.** Applicability and trade-offs.
"""


def _section_content(text: str, matches: Sequence[re.Match[str]], index: int) -> str:
    start = matches[index].end()
    end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
    return text[start:end].strip()


def _validate_learning_bullets(owner: str, content: str) -> list[str]:
    bullets = [line for line in content.splitlines() if line.startswith("- ")]
    if not bullets:
        return [f"{owner}: Learnings must contain at least one bullet"]
    if any(not LEARNING_BULLET.match(line) for line in bullets):
        return [f"{owner}: every Learnings bullet must begin with a bold takeaway"]
    return []


def validate_fragment(text: str) -> list[str]:
    chapter_matches = list(CHAPTER_HEADING.finditer(text))
    if len(chapter_matches) != 1:
        return ["fragment must contain exactly one Chapter, Chapters, Section, or Appendix H2 heading"]
    supported_h2_starts = {chapter_matches[0].start()}
    unsupported_h2 = next(
        (match.group(1).strip() for match in H2_HEADING.finditer(text) if match.start() not in supported_h2_starts),
        None,
    )
    if unsupported_h2:
        return [f"fragment contains unsupported H2 section: {unsupported_h2}"]
    owner = chapter_matches[0].group(1)
    body = text[chapter_matches[0].end() :]

    headings = list(H3_HEADING.finditer(body))
    names = tuple(match.group(1).strip() for match in headings)
    if names not in VALID_SECTION_ORDERS:
        expected = "Summary, Learnings, [Key Concepts], [Technologies & APIs], Critical Commentary"
        return [f"{owner}: section order must be {expected}"]

    errors: list[str] = []
    for index, name in enumerate(names):
        content = _section_content(body, headings, index)
        if not content:
            if name in OPTIONAL_SECTIONS:
                errors.append(f"{owner}: section '{name}' is empty; omit it")
            else:
                errors.append(f"{owner}: section '{name}' is empty")
        if name == "Learnings" and content:
            errors.extend(_validate_learning_bullets(owner, content))
    return errors


def _chapter_fragments(document: str) -> list[str]:
    chapters = list(CHAPTER_HEADING.finditer(document))
    book_wide = re.search(r"^## Book-wide Learnings\s*$", document, re.MULTILINE)
    if not book_wide:
        return []
    fragments: list[str] = []
    for index, chapter in enumerate(chapters):
        end = chapters[index + 1].start() if index + 1 < len(chapters) else book_wide.start()
        fragments.append(document[chapter.start() : end].strip())
    return fragments


def validate_document(document: str) -> list[str]:
    errors: list[str] = []
    if not re.search(r"^# What to Retain from .+ by .+$", document, re.MULTILINE):
        errors.append("document must begin with '# What to Retain from <Book> by <Author>'")
    book_wide = re.search(r"^## Book-wide Learnings\s*$", document, re.MULTILINE)
    if not book_wide:
        errors.append("document must end with '## Book-wide Learnings'")
    elif re.search(r"^## .+$", document[book_wide.end() :], re.MULTILINE):
        errors.append("Book-wide Learnings must be the document's final H2 section")
    fragments = _chapter_fragments(document)
    if not fragments:
        errors.append("document must contain at least one chapter fragment before Book-wide Learnings")
    for fragment in fragments:
        errors.extend(validate_fragment(fragment))
    supported_h2_starts = {match.start() for match in CHAPTER_HEADING.finditer(document)}
    if book_wide:
        supported_h2_starts.add(book_wide.start())
    unsupported_h2 = next(
        (
            match.group(1).strip()
            for match in H2_HEADING.finditer(document)
            if match.start() not in supported_h2_starts
            and (not book_wide or match.start() < book_wide.start())
        ),
        None,
    )
    if unsupported_h2:
        errors.append(f"document contains unsupported H2 section: {unsupported_h2}")
    if book_wide:
        content = document[book_wide.end() :].strip()
        errors.extend(_validate_learning_bullets("Book-wide Learnings", content))
    return errors


def _chapter_sort_key(fragment: str) -> tuple[int, str]:
    heading = CHAPTER_HEADING.search(fragment)
    if not heading:
        raise ContractError("chapter fragment has no recognized H2 heading")
    number = re.search(r"\b(\d+)\b", heading.group(1))
    return (int(number.group(1)) if number else sys.maxsize, heading.group(1).casefold())


def assemble_document(
    title: str,
    author: str,
    chapter_fragments: Sequence[str],
    book_wide: str,
    preamble: str | None = None,
) -> str:
    fragment_errors = [error for fragment in chapter_fragments for error in validate_fragment(fragment)]
    if fragment_errors:
        raise ContractError("; ".join(fragment_errors))
    book_wide_lines = book_wide.strip().splitlines()
    if not book_wide_lines or not re.match(r"^## Book-wide Learnings\s*$", book_wide_lines[0]):
        raise ContractError("book-wide fragment must start with '## Book-wide Learnings'")

    header = f"# What to Retain from {title} by {author}\n\n### Critically Annotated Edition"
    parts = [header]
    if preamble and preamble.strip():
        parts.append(preamble.strip())
    parts.extend(fragment.strip() for fragment in sorted(chapter_fragments, key=_chapter_sort_key))
    parts.append(book_wide.strip())
    document = "\n\n---\n\n".join(parts) + "\n"
    errors = validate_document(document)
    if errors:
        raise ContractError("; ".join(errors))
    return document


def _print_errors(errors: Sequence[str]) -> int:
    for error in errors:
        print(f"summary_contract: {error}", file=sys.stderr)
    return 1 if errors else 0


def _atomic_replace_text(path: Path, content: str) -> None:
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as temporary:
            temporary.write(content)
            temporary_path = Path(temporary.name)
        temporary_path.replace(path)
    finally:
        if temporary_path and temporary_path.exists():
            temporary_path.unlink()


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("schema")
    check = subparsers.add_parser("check")
    check.add_argument("document", type=Path)
    check_fragment = subparsers.add_parser("check-fragment")
    check_fragment.add_argument("fragment", type=Path)
    assemble = subparsers.add_parser("assemble")
    assemble.add_argument("--title", required=True)
    assemble.add_argument("--author", required=True)
    assemble.add_argument("--chapters-dir", required=True, type=Path)
    assemble.add_argument("--book-wide", required=True, type=Path)
    assemble.add_argument("--output", required=True, type=Path)
    assemble.add_argument("--preamble", type=Path)
    assemble.add_argument("--force", action="store_true")
    args = parser.parse_args(argv)

    if args.command == "schema":
        print(schema())
        return 0
    if args.command == "check":
        return _print_errors(validate_document(args.document.read_text(encoding="utf-8")))
    if args.command == "check-fragment":
        return _print_errors(validate_fragment(args.fragment.read_text(encoding="utf-8")))

    fragments = [path.read_text(encoding="utf-8") for path in sorted(args.chapters_dir.glob("ch*.md"))]
    preamble = args.preamble.read_text(encoding="utf-8") if args.preamble else None
    try:
        document = assemble_document(
            args.title,
            args.author,
            fragments,
            args.book_wide.read_text(encoding="utf-8"),
            preamble,
        )
    except ContractError as error:
        parser.exit(1, f"summary_contract: {error}\n")
    if args.output.exists() and not args.force:
        parser.exit(1, f"summary_contract: output already exists: {args.output}; use --force\n")
    if args.output.exists() and args.output.is_dir():
        parser.exit(1, f"summary_contract: output is a directory: {args.output}\n")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    _atomic_replace_text(args.output, document)
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
