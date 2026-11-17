#!/usr/bin/env python3
"""`artifact-dirs.py` gives a lone `pattern:` match its directory back (#2288)."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tests import SCRIPTS, load

artifact_dirs = load("artifact-dirs.py")


class Normalise(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name) / "shards"

    def test_a_flat_artifact_moves_into_its_name(self):
        self.root.mkdir()
        (self.root / "artifact-name").write_text("shard-3\n")
        (self.root / "caught.txt").write_text("m\n")
        (self.root / "log").mkdir()
        (self.root / "log" / "a.log").write_text("x")
        artifact_dirs.normalise(self.root)
        self.assertEqual([p.name for p in self.root.iterdir()], ["shard-3"])
        self.assertEqual((self.root / "shard-3" / "caught.txt").read_text(), "m\n")
        self.assertTrue((self.root / "shard-3" / "log" / "a.log").is_file())

    def test_an_artifact_holding_a_directory_of_its_own_name(self):
        self.root.mkdir()
        (self.root / "artifact-name").write_text("shard-0")
        (self.root / "shard-0").mkdir()
        (self.root / "shard-0" / "inner").write_text("i")
        artifact_dirs.normalise(self.root)
        self.assertTrue((self.root / "shard-0" / "shard-0" / "inner").is_file())

    def test_split_empty_and_absent_layouts_are_left_alone(self):
        artifact_dirs.normalise(self.root)
        self.assertFalse(self.root.exists())
        (self.root / "shard-0").mkdir(parents=True)
        (self.root / "shard-0" / "artifact-name").write_text("shard-0")
        artifact_dirs.normalise(self.root)
        self.assertEqual([p.name for p in self.root.iterdir()], ["shard-0"])

    def test_a_flat_artifact_with_no_or_a_bad_name_is_refused(self):
        self.root.mkdir()
        (self.root / "caught.txt").write_text("m\n")
        with self.assertRaisesRegex(ValueError, "artifact-name"):
            artifact_dirs.normalise(self.root)
        for bad in ("", "a/b", ".."):
            (self.root / "artifact-name").write_text(bad)
            with self.subTest(name=bad), self.assertRaises(ValueError):
                artifact_dirs.normalise(self.root)
        self.assertTrue((self.root / "caught.txt").is_file())   # nothing moved

    def test_the_cli_fails_red_with_what_to_do(self):
        self.root.mkdir()
        (self.root / "caught.txt").write_text("m\n")
        done = subprocess.run([sys.executable, str(SCRIPTS / "artifact-dirs.py"), str(self.root)],
                              capture_output=True, text=True, check=False)
        self.assertEqual(done.returncode, 1)
        self.assertIn("::error::", done.stderr)
        self.assertIn("upload-artifact", done.stderr)


if __name__ == "__main__":
    unittest.main()
