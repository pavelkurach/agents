import importlib.util
import json
import tempfile
import unittest
import zipfile
from pathlib import Path


SCRIPT_PATH = Path(__file__).parents[1] / "scripts" / "prepare_book.py"
SPEC = importlib.util.spec_from_file_location("prepare_book", SCRIPT_PATH)
prepare_book = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(prepare_book)


def write_epub(path: Path) -> None:
    container = """\
<?xml version="1.0"?>
<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles>
</container>
"""
    package = """\
<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Reliable Systems</dc:title><dc:creator>Ada Example</dc:creator>
  </metadata>
  <manifest>
    <item id="one" href="one.xhtml" media-type="application/xhtml+xml"/>
    <item id="two" href="two.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="one"/><itemref idref="two"/></spine>
</package>
"""
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("META-INF/container.xml", container)
        archive.writestr("OEBPS/content.opf", package)
        archive.writestr(
            "OEBPS/one.xhtml",
            "<html><head><title>Reliable Systems</title></head>"
            "<body><h1>Queues</h1><p>Bound queued work.</p></body></html>",
        )
        archive.writestr(
            "OEBPS/two.xhtml",
            "<html><body><h1>Retries</h1><p>Retry only idempotent work.</p></body></html>",
        )


class PrepareBookTests(unittest.TestCase):
    def test_pdf_outline_keeps_chapters_and_closing_material_not_subsections(self) -> None:
        entries = (
            prepare_book.OutlineEntry("Chapter 1: Queues", 1, 0),
            prepare_book.OutlineEntry("Summary", 3, 1),
            prepare_book.OutlineEntry("Implementation", 4, 1),
            prepare_book.OutlineEntry("Chapter 2: Retries", 7, 0),
            prepare_book.OutlineEntry("Summary", 9, 1),
            prepare_book.OutlineEntry("Appendix A: Reference", 11, 0),
        )

        self.assertEqual(
            (
                ("Chapter 1: Queues", 1),
                ("Chapter 2: Retries", 7),
                ("Appendix A: Reference", 11),
            ),
            prepare_book.select_pdf_boundaries(entries),
        )

    def test_pdf_bookmarks_define_section_ranges(self) -> None:
        sections = prepare_book.plan_pdf_sections(
            page_texts=("first", "second", "third", "fourth"),
            bookmarks=(("Queues", 0), ("Retries", 2)),
            chunk_pages=30,
        )

        self.assertEqual(
            (
                prepare_book.Section("Queues", "first\n\nsecond", "pages 1-2"),
                prepare_book.Section("Retries", "third\n\nfourth", "pages 3-4"),
            ),
            sections,
        )

    def test_pdf_heading_scan_preserves_front_matter(self) -> None:
        sections = prepare_book.plan_pdf_sections(
            page_texts=(
                "Copyright and preface",
                "Chapter 1\nQueues",
                "Bound queued work.",
                "Chapter 2: Retries\nRetry carefully.",
            ),
            bookmarks=(),
            chunk_pages=30,
        )

        self.assertEqual("Front matter", sections[0].title)
        self.assertEqual("Chapter 1: Queues", sections[1].title)
        self.assertEqual("Chapter 2: Retries", sections[2].title)

    def test_pdf_without_structure_falls_back_to_equal_chunks(self) -> None:
        sections = prepare_book.plan_pdf_sections(
            page_texts=("one", "two", "three", "four", "five"),
            bookmarks=(),
            chunk_pages=2,
        )

        self.assertEqual(("Section 1", "Section 2", "Section 3"), tuple(s.title for s in sections))
        self.assertEqual("pages 5-5", sections[2].locator)

    def test_read_epub_uses_spine_order_and_document_headings(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "book.epub"
            write_epub(path)

            book = prepare_book.read_epub(path)

        self.assertEqual("Reliable Systems", book.title)
        self.assertEqual("Ada Example", book.author)
        self.assertEqual(("Queues", "Retries"), tuple(s.title for s in book.sections))
        self.assertIn("Bound queued work.", book.sections[0].text)

    def test_prepare_book_writes_manifest_and_chapter_texts(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "book.epub"
            work_dir = root / "work"
            write_epub(source)

            manifest_path = prepare_book.prepare(source, work_dir)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            chapter_text = (work_dir / manifest["sections"][0]["text_path"]).read_text(
                encoding="utf-8"
            )

        self.assertEqual("Reliable Systems", manifest["title"])
        self.assertEqual("Ada Example", manifest["author"])
        self.assertEqual("chapters/ch01.txt", manifest["sections"][0]["text_path"])
        self.assertEqual("Queues\n\nBound queued work.\n", chapter_text)

    def test_prepare_rejects_unsupported_input(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            source = Path(temp_dir) / "book.txt"
            source.write_text("not a book", encoding="utf-8")

            with self.assertRaisesRegex(prepare_book.PrepareError, "expected a PDF or EPUB"):
                prepare_book.prepare(source, Path(temp_dir) / "work")

    def test_force_refuses_to_reset_an_unowned_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "book.epub"
            work_dir = root / "work"
            work_dir.mkdir()
            sentinel = work_dir / "keep.txt"
            sentinel.write_text("keep", encoding="utf-8")
            write_epub(source)

            with self.assertRaisesRegex(prepare_book.PrepareError, "not owned by this skill"):
                prepare_book.prepare(source, work_dir, force=True)

            self.assertEqual("keep", sentinel.read_text(encoding="utf-8"))

    def test_force_refuses_to_reset_a_symlinked_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "book.epub"
            target = root / "target"
            link = root / "linked-work"
            write_epub(source)
            prepare_book.prepare(source, target)
            link.symlink_to(target, target_is_directory=True)
            sentinel = target / "keep.txt"
            sentinel.write_text("keep", encoding="utf-8")

            with self.assertRaisesRegex(prepare_book.PrepareError, "symlinked work directory"):
                prepare_book.prepare(source, link, force=True)

            self.assertEqual("keep", sentinel.read_text(encoding="utf-8"))

    def test_prepare_refuses_an_unowned_nonempty_work_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "book.epub"
            work_dir = root / "work"
            chapters = work_dir / "chapters"
            chapters.mkdir(parents=True)
            sentinel = chapters / "ch01.txt"
            sentinel.write_text("keep", encoding="utf-8")
            write_epub(source)

            with self.assertRaisesRegex(prepare_book.PrepareError, "not empty"):
                prepare_book.prepare(source, work_dir)

            self.assertEqual("keep", sentinel.read_text(encoding="utf-8"))

    def test_force_can_reset_a_previously_prepared_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "book.epub"
            work_dir = root / "work"
            write_epub(source)
            prepare_book.prepare(source, work_dir)
            stale_file = work_dir / "stale.txt"
            stale_file.write_text("stale", encoding="utf-8")

            manifest_path = prepare_book.prepare(source, work_dir, force=True)

            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual("summarize-technical-book", manifest["prepared_by"])
            self.assertFalse(stale_file.exists())


if __name__ == "__main__":
    unittest.main()
