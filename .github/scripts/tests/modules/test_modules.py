"""A moved module reads at its module path, as `tools/lint/src/modules.rs` reads it."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import modules


class ModulePath(unittest.TestCase):
    def test_a_moved_file_reads_at_its_module_path(self):
        self.assertEqual(modules.module_path("crates/core/src/lib.rs"), "src/core/mod.rs")
        self.assertEqual(modules.module_path("crates/core/src/fs/mod.rs"), "src/core/fs/mod.rs")
        self.assertEqual(modules.module_path("crates/core/src/"), "src/core/")

    def test_other_paths_keep_their_own(self):
        for path in ("src/scene/mod.rs", "crates/core/build.rs", "crates/core/Cargo.toml",
                     "crates/test-support/src/temp.rs", "tests/main.rs", "cratesx/a/src/lib.rs"):
            with self.subTest(path=path):
                self.assertEqual(modules.module_path(path), path)


class Tree(unittest.TestCase):
    def test_a_module_is_found_where_it_lives(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for d in ("crates/core/src", "crates/test-support/src", "src/scene"):
                (root / d).mkdir(parents=True)
            self.assertEqual(modules.moved(root), ["core"])
            self.assertEqual(modules.module_dir(root, "core"), root / "crates/core/src")
            self.assertEqual(modules.module_dir(root, "scene"), root / "src/scene")


if __name__ == "__main__":
    unittest.main()
