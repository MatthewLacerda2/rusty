"""#1569: `make review-level` says `high` for the table's paths and `medium` otherwise."""

from __future__ import annotations

import contextlib
import io
import unittest
from unittest import mock

from dispatch import review_level
from tests import SCRIPTS, load

ROOT = SCRIPTS.parent.parent
cli = load("review-level.py")


class Level(unittest.TestCase):
    def test_each_responsibility_calls_for_high(self):
        cases = {
            "stored format": "src/scene/prefab/apply.rs",
            "determinism": "clippy.toml",
            "scripting sandbox": "src/scripting/sandbox/files.rs",
            "merge path": ".github/workflows/ci.yml",
        }
        for why, path in cases.items():
            with self.subTest(why):
                self.assertEqual(review_level.level(["README.md", path]), ("high", [(why, path)]))

    def test_everything_else_is_medium(self):
        paths = ["src/render/post/bloom.rs", "docs/api/Scene.md", ".github/scripts/board.py",
                 "src/scene/io_helpers.rs", "src/scripting/sandboxing.rs"]
        self.assertEqual(review_level.level(paths), ("medium", []))

    def test_a_file_entry_matches_only_itself(self):
        self.assertEqual(review_level.reasons_for("src/scene/serialize.rs"), ["stored format"])
        self.assertEqual(review_level.reasons_for("src/scene/serialize_tests.rs"), [])

    def test_a_rename_counts_its_old_name(self):
        files = [{"filename": "src/scene/moved.rs", "previous_filename": "src/scene/serialize.rs"},
                 {"filename": "README.md"}]
        self.assertEqual(review_level.changed_paths(files),
                         ["src/scene/moved.rs", "src/scene/serialize.rs", "README.md"])

    def test_every_entry_of_the_table_exists(self):
        """A path that moved leaves its row matching nothing: the table would go quiet."""
        for why, entries in review_level.TABLE.items():
            for entry in entries:
                with self.subTest(entry):
                    target = ROOT / entry.rstrip("/")
                    self.assertTrue(target.is_dir() if entry.endswith("/") else target.is_file(),
                                    f"{why}: {entry} no longer exists; update the table")


class CommandLine(unittest.TestCase):
    def run_cli(self, files: list[dict] | None) -> tuple[int, str]:
        out = io.StringIO()
        gh = lambda path: files if "page=1" in path else []  # noqa: E731
        with mock.patch.object(cli.ghrest, "slug", return_value="o/r"), \
                contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
            code = cli.main(["12"], gh=gh if files is not None else (lambda path: None))
        return code, out.getvalue()

    def test_prints_the_level_then_why(self):
        code, out = self.run_cli([{"filename": ".github/scripts/merging/queue/land.py"}])
        self.assertEqual(code, 0)
        self.assertEqual(out.splitlines(), ["high", "  merge path: .github/scripts/merging/queue/land.py"])

    def test_medium_says_nothing_matched(self):
        code, out = self.run_cli([{"filename": "src/render/x.rs"}])
        self.assertEqual((code, out.splitlines()[0]), (0, "medium"))
        self.assertIn("no path of the table", out)

    def test_an_unreadable_pr_exits_2(self):
        self.assertEqual(self.run_cli(None)[0], 2)


if __name__ == "__main__":
    unittest.main()
