#!/usr/bin/env python3
"""Extract a PDF or EPUB into stable, resumable chapter text files."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import tempfile
import zipfile
from html.parser import HTMLParser
from pathlib import Path, PurePosixPath
from typing import NamedTuple, Sequence
from xml.etree import ElementTree


class PrepareError(RuntimeError):
    pass


class Section(NamedTuple):
    title: str
    text: str
    locator: str


class Book(NamedTuple):
    title: str
    author: str
    format: str
    sections: tuple[Section, ...]
    warnings: tuple[str, ...] = ()


class OutlineEntry(NamedTuple):
    title: str
    page_index: int
    depth: int


class _HTMLText(HTMLParser):
    BLOCK_TAGS = frozenset(
        {"article", "blockquote", "br", "div", "h1", "h2", "h3", "h4", "li", "p", "section"}
    )
    IGNORED_TAGS = frozenset({"head", "script", "style"})

    def __init__(self) -> None:
        super().__init__()
        self.parts: list[str] = []
        self.heading_parts: list[str] = []
        self._heading_depth = 0
        self._ignored_depth = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        del attrs
        if tag in self.IGNORED_TAGS:
            self._ignored_depth += 1
        if tag in {"h1", "h2"} and not self.heading_parts and not self._ignored_depth:
            self._heading_depth += 1
        if tag in self.BLOCK_TAGS and not self._ignored_depth:
            self.parts.append("\n")

    def handle_endtag(self, tag: str) -> None:
        if tag in {"h1", "h2"} and self._heading_depth:
            self._heading_depth -= 1
        if tag in self.BLOCK_TAGS and not self._ignored_depth:
            self.parts.append("\n")
        if tag in self.IGNORED_TAGS and self._ignored_depth:
            self._ignored_depth -= 1

    def handle_data(self, data: str) -> None:
        if self._ignored_depth:
            return
        self.parts.append(data)
        if self._heading_depth:
            self.heading_parts.append(data)

    @property
    def text(self) -> str:
        lines = (" ".join(line.split()) for line in "".join(self.parts).splitlines())
        return "\n\n".join(line for line in lines if line)

    @property
    def heading(self) -> str:
        return " ".join("".join(self.heading_parts).split())


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _child_text(element: ElementTree.Element, local_name: str) -> str:
    for child in element.iter():
        if _local_name(child.tag) == local_name and child.text:
            return " ".join(child.text.split())
    return ""


def _parse_html(data: bytes) -> tuple[str, str]:
    parser = _HTMLText()
    parser.feed(data.decode("utf-8", errors="replace"))
    return parser.heading, parser.text


def read_epub(path: Path) -> Book:
    try:
        archive = zipfile.ZipFile(path)
    except (OSError, zipfile.BadZipFile) as error:
        raise PrepareError(f"cannot read EPUB {path}: {error}") from error

    with archive:
        try:
            container = ElementTree.fromstring(archive.read("META-INF/container.xml"))
            rootfile = next(
                element
                for element in container.iter()
                if _local_name(element.tag) == "rootfile"
            )
            package_path = PurePosixPath(rootfile.attrib["full-path"])
            package = ElementTree.fromstring(archive.read(str(package_path)))
        except (KeyError, StopIteration, ElementTree.ParseError) as error:
            raise PrepareError(f"EPUB {path} has no readable package document") from error

        manifest: dict[str, str] = {}
        spine_ids: list[str] = []
        for element in package.iter():
            name = _local_name(element.tag)
            if name == "item" and "id" in element.attrib and "href" in element.attrib:
                manifest[element.attrib["id"]] = element.attrib["href"]
            if name == "itemref" and "idref" in element.attrib:
                spine_ids.append(element.attrib["idref"])

        package_dir = package_path.parent
        sections: list[Section] = []
        for item_id in spine_ids:
            href = manifest.get(item_id)
            if not href:
                continue
            document_path = package_dir / PurePosixPath(href.split("#", 1)[0])
            try:
                heading, text = _parse_html(archive.read(str(document_path)))
            except KeyError as error:
                raise PrepareError(f"EPUB spine item is missing: {document_path}") from error
            if not text:
                continue
            title = heading or PurePosixPath(href).stem.replace("_", " ").replace("-", " ").title()
            sections.append(Section(title, text, str(document_path)))

    if not sections:
        raise PrepareError(f"EPUB {path} contains no readable spine documents")
    title = _child_text(package, "title") or path.stem
    author = _child_text(package, "creator")
    return Book(title, author, "epub", tuple(sections))


_CHAPTER_LINE = re.compile(r"^chapter\s+(\d+)\s*[:.\-]?\s*(.*)$", re.IGNORECASE)
_NUMBERED_LINE = re.compile(r"^(\d+)\.\s+([A-Z][^\n]{2,})$")
_CHAPTER_BOOKMARK = re.compile(
    r"^(?:chapter\s+(?:\d+|[ivxlcdm]+)\b|(?:\d+|[ivxlcdm]+)(?:\.\s+|:\s+|-\s+|\s+)\S)",
    re.IGNORECASE,
)
_CLOSING_BOOKMARK = re.compile(
    r"^(?:appendix(?:\s+[A-Z0-9]+)?|afterword|bibliography|references|index)\b",
    re.IGNORECASE,
)


def _page_heading(text: str) -> str | None:
    lines = [" ".join(line.split()) for line in text.splitlines()[:10] if line.strip()]
    for index, line in enumerate(lines):
        chapter_match = _CHAPTER_LINE.match(line)
        if chapter_match:
            number, title = chapter_match.groups()
            if not title and index + 1 < len(lines):
                title = lines[index + 1]
            return f"Chapter {number}" + (f": {title}" if title else "")
        numbered_match = _NUMBERED_LINE.match(line)
        if numbered_match:
            return f"Chapter {numbered_match.group(1)}: {numbered_match.group(2)}"
    return None


def _sections_from_boundaries(
    page_texts: Sequence[str], boundaries: Sequence[tuple[str, int]]
) -> tuple[Section, ...]:
    normalized: list[tuple[str, int]] = []
    seen_pages: set[int] = set()
    for title, page_index in sorted(boundaries, key=lambda item: item[1]):
        if page_index < 0 or page_index >= len(page_texts) or page_index in seen_pages:
            continue
        normalized.append((" ".join(title.split()) or f"Section {len(normalized) + 1}", page_index))
        seen_pages.add(page_index)
    if not normalized:
        return ()
    if normalized[0][1] > 0:
        normalized.insert(0, ("Front matter", 0))

    sections: list[Section] = []
    for index, (title, start) in enumerate(normalized):
        end = normalized[index + 1][1] if index + 1 < len(normalized) else len(page_texts)
        text = "\n\n".join(page.strip() for page in page_texts[start:end] if page.strip())
        if text:
            sections.append(Section(title, text, f"pages {start + 1}-{end}"))
    return tuple(sections)


def plan_pdf_sections(
    page_texts: Sequence[str],
    bookmarks: Sequence[tuple[str, int]],
    chunk_pages: int,
) -> tuple[Section, ...]:
    if chunk_pages < 1:
        raise PrepareError("chunk_pages must be positive")
    from_bookmarks = _sections_from_boundaries(page_texts, bookmarks)
    if from_bookmarks:
        return from_bookmarks

    headings = tuple(
        (heading, page_index)
        for page_index, text in enumerate(page_texts)
        if (heading := _page_heading(text)) is not None
    )
    from_headings = _sections_from_boundaries(page_texts, headings)
    if len(headings) >= 2 and from_headings:
        return from_headings

    chunks: list[Section] = []
    for start in range(0, len(page_texts), chunk_pages):
        end = min(start + chunk_pages, len(page_texts))
        text = "\n\n".join(page.strip() for page in page_texts[start:end] if page.strip())
        if text:
            chunks.append(Section(f"Section {len(chunks) + 1}", text, f"pages {start + 1}-{end}"))
    return tuple(chunks)


def select_pdf_boundaries(entries: Sequence[OutlineEntry]) -> tuple[tuple[str, int], ...]:
    chapters = [entry for entry in entries if _CHAPTER_BOOKMARK.match(entry.title)]
    if len(chapters) >= 2:
        last_chapter_page = max(entry.page_index for entry in chapters)
        closing = [
            entry
            for entry in entries
            if entry.page_index > last_chapter_page and _CLOSING_BOOKMARK.match(entry.title)
        ]
        selected = chapters + closing
        return tuple((entry.title, entry.page_index) for entry in selected)

    for depth in sorted({entry.depth for entry in entries}):
        at_depth = [entry for entry in entries if entry.depth == depth]
        if len({entry.page_index for entry in at_depth}) >= 2:
            return tuple((entry.title, entry.page_index) for entry in at_depth)
    return tuple((entry.title, entry.page_index) for entry in entries)


def _pdf_bookmarks(reader: object) -> tuple[tuple[str, int], ...]:
    entries: list[OutlineEntry] = []

    def visit(items: object, depth: int) -> None:
        if not isinstance(items, list):
            return
        for item in items:
            if isinstance(item, list):
                visit(item, depth + 1)
                continue
            try:
                page_index = reader.get_destination_page_number(item)  # type: ignore[attr-defined]
            except (AttributeError, KeyError, ValueError):
                continue
            title = str(getattr(item, "title", "")).strip()
            if title and isinstance(page_index, int):
                entries.append(OutlineEntry(title, page_index, depth))

    visit(getattr(reader, "outline", []), 0)
    return select_pdf_boundaries(entries)


def read_pdf(path: Path, chunk_pages: int) -> Book:
    try:
        from pypdf import PdfReader
    except ImportError as error:
        raise PrepareError("PDF support requires pypdf; run with 'uv run --with pypdf'") from error

    try:
        reader = PdfReader(path)
        page_texts = tuple((page.extract_text() or "").strip() for page in reader.pages)
    except Exception as error:
        raise PrepareError(f"cannot read PDF {path}: {error}") from error
    if not any(page_texts):
        raise PrepareError("PDF appears image-only; run OCR and retry")

    bookmarks = _pdf_bookmarks(reader)
    sections = plan_pdf_sections(page_texts, bookmarks, chunk_pages)
    if not sections:
        raise PrepareError(f"PDF {path} contains no extractable text")
    metadata = reader.metadata or {}
    title = str(metadata.get("/Title") or path.stem)
    author = str(metadata.get("/Author") or "")
    warning = () if bookmarks else ("No usable PDF outline; inferred sections.",)
    return Book(title, author, "pdf", sections, warning)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _slug(text: str) -> str:
    value = re.sub(r"[^a-z0-9]+", "-", text.casefold()).strip("-")
    return value or "book"


def default_work_dir(source: Path) -> Path:
    fingerprint = _sha256(source)[:12]
    return Path(tempfile.gettempdir()) / "summarize-technical-book" / f"{_slug(source.stem)}-{fingerprint}"


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


def _safe_reset(work_dir: Path) -> None:
    if work_dir.is_symlink():
        raise PrepareError(f"refusing to reset symlinked work directory: {work_dir}")
    resolved = work_dir.resolve()
    forbidden = {Path("/").resolve(), Path.home().resolve(), Path(tempfile.gettempdir()).resolve()}
    if resolved in forbidden:
        raise PrepareError(f"refusing to reset broad work directory: {resolved}")
    if not resolved.exists():
        return
    if not resolved.is_dir():
        raise PrepareError(f"work directory path is not a directory: {resolved}")
    if not any(resolved.iterdir()):
        resolved.rmdir()
        return
    manifest_path = resolved / "manifest.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PrepareError(f"refusing to reset directory not owned by this skill: {resolved}") from error
    if manifest.get("prepared_by") != "summarize-technical-book":
        raise PrepareError(f"refusing to reset directory not owned by this skill: {resolved}")
    shutil.rmtree(resolved)


def _kind(title: str) -> str:
    lowered = title.casefold()
    if lowered.startswith(("appendix", "afterword", "bibliography", "references", "index")):
        return "back_matter"
    if lowered.startswith(("front matter", "preface", "foreword", "acknowledg")):
        return "front_matter"
    return "chapter"


def prepare(source: Path, work_dir: Path | None = None, force: bool = False, chunk_pages: int = 30) -> Path:
    source = source.expanduser().resolve()
    if not source.is_file() or source.suffix.casefold() not in {".pdf", ".epub"}:
        raise PrepareError(f"expected a PDF or EPUB file: {source}")
    source_hash = _sha256(source)
    requested_destination = (work_dir or default_work_dir(source)).expanduser()
    if requested_destination.is_symlink():
        raise PrepareError(f"refusing to use symlinked work directory: {requested_destination}")
    destination = requested_destination.resolve()
    if source == destination or destination in source.parents:
        raise PrepareError("source file must be outside the work directory")
    manifest_path = destination / "manifest.json"
    if destination.exists() and not manifest_path.exists() and not force:
        if not destination.is_dir():
            raise PrepareError(f"work directory path is not a directory: {destination}")
        if any(destination.iterdir()):
            raise PrepareError(f"work directory is not empty and has no skill manifest: {destination}")
    if manifest_path.exists() and not force:
        try:
            existing = json.loads(manifest_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as error:
            raise PrepareError(f"cannot read work manifest {manifest_path}: {error}") from error
        if existing.get("prepared_by") != "summarize-technical-book":
            raise PrepareError(f"work directory is not owned by this skill: {destination}")
        if existing.get("source_sha256") == source_hash:
            return manifest_path
        raise PrepareError(f"work directory belongs to different source: {destination}; use --force")
    if force:
        _safe_reset(destination)

    book = read_pdf(source, chunk_pages) if source.suffix.casefold() == ".pdf" else read_epub(source)
    chapters_dir = destination / "chapters"
    chapters_dir.mkdir(parents=True, exist_ok=True)

    section_records: list[dict[str, str | int]] = []
    for number, section in enumerate(book.sections, start=1):
        relative_path = Path("chapters") / f"ch{number:02d}.txt"
        (destination / relative_path).write_text(section.text.rstrip() + "\n", encoding="utf-8")
        section_records.append(
            {
                "number": number,
                "title": section.title,
                "kind": _kind(section.title),
                "locator": section.locator,
                "text_path": relative_path.as_posix(),
            }
        )

    manifest = {
        "prepared_by": "summarize-technical-book",
        "source": str(source),
        "source_sha256": source_hash,
        "format": book.format,
        "title": book.title,
        "author": book.author,
        "warnings": list(book.warnings),
        "sections": section_records,
    }
    destination.mkdir(parents=True, exist_ok=True)
    _atomic_replace_text(
        manifest_path,
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
    )
    return manifest_path


def manifest_schema() -> str:
    return json.dumps(
        {
            "prepared_by": "summarize-technical-book",
            "source": "/absolute/path/book.pdf",
            "source_sha256": "…",
            "format": "pdf | epub",
            "title": "Book title",
            "author": "Author",
            "warnings": [],
            "sections": [
                {
                    "number": 1,
                    "title": "Chapter title",
                    "kind": "chapter | front_matter | back_matter",
                    "locator": "pages 1-20 | EPUB/path.xhtml",
                    "text_path": "chapters/ch01.txt",
                }
            ],
        },
        indent=2,
        ensure_ascii=False,
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", nargs="?", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--chunk-pages", type=int, default=30)
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--schema", action="store_true")
    args = parser.parse_args(argv)
    if args.schema:
        print(manifest_schema())
        return 0
    if args.source is None:
        parser.error("source is required unless --schema is used")
    try:
        print(prepare(args.source, args.work_dir, args.force, args.chunk_pages))
    except PrepareError as error:
        parser.exit(1, f"prepare_book: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
