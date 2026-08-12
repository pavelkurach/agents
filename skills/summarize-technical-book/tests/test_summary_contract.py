import importlib.util
import io
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path


SCRIPT_PATH = Path(__file__).parents[1] / "scripts" / "summary_contract.py"
SPEC = importlib.util.spec_from_file_location("summary_contract", SCRIPT_PATH)
summary_contract = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(summary_contract)


VALID_FRAGMENT = """\
## Chapter 1: Backpressure

### Summary

Fast producers can overwhelm slower consumers unless the system bounds queued work.

### Learnings

- **Bound every queue you operate.** A finite buffer turns overload into a visible policy decision instead of hidden memory growth.

### Key Concepts

- **Backpressure**: A mechanism that makes producers slow down or fail when consumers cannot keep pace.

### Critical Commentary

The chapter does not compare overload policies. [model-claim]
"""

BOOK_WIDE = """\
## Book-wide Learnings

- **Make overload behavior explicit.** Bounded queues force the system to choose between waiting, rejecting, or shedding work.
"""


class SummaryContractTests(unittest.TestCase):
    def test_valid_fragment_allows_omitted_technologies_section(self) -> None:
        self.assertEqual([], summary_contract.validate_fragment(VALID_FRAGMENT))

    def test_valid_fragment_allows_omitted_key_concepts_section(self) -> None:
        fragment = VALID_FRAGMENT.replace(
            "### Key Concepts\n\n- **Backpressure**: A mechanism that makes producers slow down or fail when consumers cannot keep pace.\n\n",
            "",
        )

        self.assertEqual([], summary_contract.validate_fragment(fragment))

    def test_valid_fragment_allows_nonempty_technologies_section_in_order(self) -> None:
        fragment = VALID_FRAGMENT.replace(
            "### Critical Commentary",
            "### Technologies & APIs\n\n- **Tokio channels**: Bounded channels expose queue capacity at construction.\n\n### Critical Commentary",
        )

        self.assertEqual([], summary_contract.validate_fragment(fragment))

    def test_fragment_rejects_an_unknown_section(self) -> None:
        fragment = VALID_FRAGMENT.replace(
            "### Key Concepts",
            "### Exercises\n\n1. Model the overload policy.\n\n### Key Concepts",
        )

        self.assertEqual(
            [
                "Chapter 1: Backpressure: section order must be Summary, Learnings, "
                "[Key Concepts], [Technologies & APIs], Critical Commentary"
            ],
            summary_contract.validate_fragment(fragment),
        )

    def test_fragment_rejects_an_extra_h2_section(self) -> None:
        fragment = VALID_FRAGMENT.replace(
            "### Summary",
            "## Exercises\n\nModel the overload policy.\n\n### Summary",
        )

        self.assertEqual(
            ["fragment contains unsupported H2 section: Exercises"],
            summary_contract.validate_fragment(fragment),
        )

    def test_fragment_rejects_empty_technologies_section(self) -> None:
        fragment = VALID_FRAGMENT.replace(
            "### Critical Commentary",
            "### Technologies & APIs\n\n### Critical Commentary",
        )

        self.assertEqual(
            ["Chapter 1: Backpressure: section 'Technologies & APIs' is empty; omit it"],
            summary_contract.validate_fragment(fragment),
        )

    def test_fragment_rejects_empty_key_concepts_section(self) -> None:
        fragment = VALID_FRAGMENT.replace(
            "- **Backpressure**: A mechanism that makes producers slow down or fail when consumers cannot keep pace.\n\n",
            "",
        )

        self.assertEqual(
            ["Chapter 1: Backpressure: section 'Key Concepts' is empty; omit it"],
            summary_contract.validate_fragment(fragment),
        )

    def test_fragment_rejects_learning_without_bold_takeaway(self) -> None:
        fragment = VALID_FRAGMENT.replace(
            "- **Bound every queue you operate.**",
            "- Bound every queue you operate.",
        )

        self.assertEqual(
            ["Chapter 1: Backpressure: every Learnings bullet must begin with a bold takeaway"],
            summary_contract.validate_fragment(fragment),
        )

    def test_assemble_document_orders_fragments_and_validates_result(self) -> None:
        chapter_two = VALID_FRAGMENT.replace("Chapter 1", "Chapter 2")

        document = summary_contract.assemble_document(
            title="Systems That Last",
            author="Ada Example",
            chapter_fragments=[chapter_two, VALID_FRAGMENT],
            book_wide=BOOK_WIDE,
        )

        self.assertLess(document.index("## Chapter 1"), document.index("## Chapter 2"))
        self.assertEqual([], summary_contract.validate_document(document))

    def test_assemble_rejects_empty_book_wide_fragment_cleanly(self) -> None:
        with self.assertRaisesRegex(summary_contract.ContractError, "book-wide fragment"):
            summary_contract.assemble_document(
                title="Systems That Last",
                author="Ada Example",
                chapter_fragments=[VALID_FRAGMENT],
                book_wide="",
            )

    def test_cli_check_reports_invalid_document(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "invalid.md"
            path.write_text(VALID_FRAGMENT, encoding="utf-8")

            errors = io.StringIO()
            with redirect_stderr(errors):
                exit_code = summary_contract.main(["check", str(path)])

        self.assertEqual(1, exit_code)
        self.assertIn("document must begin", errors.getvalue())

    def test_document_rejects_content_after_book_wide_learnings(self) -> None:
        document = summary_contract.assemble_document(
            title="Systems That Last",
            author="Ada Example",
            chapter_fragments=[VALID_FRAGMENT],
            book_wide=BOOK_WIDE,
        )

        self.assertEqual(
            ["Book-wide Learnings must be the document's final H2 section"],
            summary_contract.validate_document(document + "\n## Exercises\n\n1. Model overload.\n"),
        )

    def test_document_rejects_an_extra_h2_in_preamble(self) -> None:
        document = summary_contract.assemble_document(
            title="Systems That Last",
            author="Ada Example",
            chapter_fragments=[VALID_FRAGMENT],
            book_wide=BOOK_WIDE,
        )
        document = document.replace(
            "### Critically Annotated Edition",
            "### Critically Annotated Edition\n\n## Reading Notes\n\nSource context.",
        )

        self.assertEqual(
            ["document contains unsupported H2 section: Reading Notes"],
            summary_contract.validate_document(document),
        )

    def test_cli_assemble_does_not_overwrite_without_force(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fragments = root / "fragments"
            fragments.mkdir()
            (fragments / "ch01.md").write_text(VALID_FRAGMENT, encoding="utf-8")
            book_wide = root / "book-wide.md"
            book_wide.write_text(BOOK_WIDE, encoding="utf-8")
            output = root / "summary.md"
            output.write_text("keep", encoding="utf-8")

            errors = io.StringIO()
            with redirect_stderr(errors), self.assertRaises(SystemExit) as raised:
                summary_contract.main(
                    [
                        "assemble",
                        "--title",
                        "Systems That Last",
                        "--author",
                        "Ada Example",
                        "--chapters-dir",
                        str(fragments),
                        "--book-wide",
                        str(book_wide),
                        "--output",
                        str(output),
                    ]
                )

            self.assertEqual(1, raised.exception.code)
            self.assertIn("already exists", errors.getvalue())
            self.assertEqual("keep", output.read_text(encoding="utf-8"))

    def test_cli_assemble_does_not_follow_a_predictable_temp_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fragments = root / "fragments"
            fragments.mkdir()
            (fragments / "ch01.md").write_text(VALID_FRAGMENT, encoding="utf-8")
            book_wide = root / "book-wide.md"
            book_wide.write_text(BOOK_WIDE, encoding="utf-8")
            output = root / "summary.md"
            victim = root / "victim.txt"
            victim.write_text("keep", encoding="utf-8")
            output.with_suffix(".md.tmp").symlink_to(victim)

            with redirect_stdout(io.StringIO()):
                exit_code = summary_contract.main(
                    [
                        "assemble",
                        "--title",
                        "Systems That Last",
                        "--author",
                        "Ada Example",
                        "--chapters-dir",
                        str(fragments),
                        "--book-wide",
                        str(book_wide),
                        "--output",
                        str(output),
                    ]
                )

            self.assertEqual(0, exit_code)
            self.assertEqual("keep", victim.read_text(encoding="utf-8"))
            self.assertFalse(output.is_symlink())


if __name__ == "__main__":
    unittest.main()
