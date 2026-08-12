import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).parents[1] / "scripts" / "concept_inventory.py"
SPEC = importlib.util.spec_from_file_location("concept_inventory", SCRIPT_PATH)
concept_inventory = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(concept_inventory)


class ConceptInventoryTests(unittest.TestCase):
    def test_collate_groups_normalized_names_and_preserves_chapter_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            inventory_dir = Path(temp_dir)
            (inventory_dir / "ch01.json").write_text(
                json.dumps(
                    {
                        "concepts": [
                            {
                                "name": "Backpressure",
                                "role": "introduced",
                                "brief_take": "Bounds work when consumers lag.",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            (inventory_dir / "ch02.json").write_text(
                json.dumps(
                    {
                        "concepts": [
                            {
                                "name": " backpressure ",
                                "role": "elaborated",
                                "brief_take": "Connects overload to admission control.",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            result = concept_inventory.collate_directory(inventory_dir)

        self.assertEqual(
            {
                "concepts": [
                    {
                        "name": "Backpressure",
                        "occurrences": [
                            {
                                "chapter": "ch01",
                                "role": "introduced",
                                "brief_take": "Bounds work when consumers lag.",
                            },
                            {
                                "chapter": "ch02",
                                "role": "elaborated",
                                "brief_take": "Connects overload to admission control.",
                            },
                        ],
                    }
                ]
            },
            result,
        )

    def test_collate_rejects_unsupported_role(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            inventory_dir = Path(temp_dir)
            (inventory_dir / "ch01.json").write_text(
                json.dumps(
                    {
                        "concepts": [
                            {
                                "name": "Tokio select",
                                "role": "mentioned",
                                "brief_take": "Races branches.",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                concept_inventory.InventoryError,
                r"ch01\.json: concepts\[0\]\.role must be 'introduced' or 'elaborated'",
            ):
                concept_inventory.collate_directory(inventory_dir)

    def test_collate_rejects_missing_brief_take(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            inventory_dir = Path(temp_dir)
            (inventory_dir / "ch01.json").write_text(
                json.dumps(
                    {
                        "concepts": [
                            {
                                "name": "Backpressure",
                                "role": "introduced",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                concept_inventory.InventoryError,
                r"ch01\.json: concepts\[0\]\.brief_take must be a non-empty string",
            ):
                concept_inventory.collate_directory(inventory_dir)


if __name__ == "__main__":
    unittest.main()
