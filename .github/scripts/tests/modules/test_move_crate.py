"""`make move-crate` makes a move's mechanical edits and lists the rest (crates/CLAUDE.md)."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from tests import load

mover = load("move_crate.py")

CARGO = """[workspace.dependencies]
rusty-core = { path = "crates/core" }
rusty-test-support = { path = "crates/test-support" }
glam = { version = "0.33" }
rustc-hash = "2"
naga_oil = "0.23"

[features]
dev = ["rusty-core/dev", "dep:x"]

[dependencies]
# The moved modules.
rusty-core = { workspace = true }
glam = { workspace = true }
"""
LIB = "pub mod asset;\npub mod scene;\n\npub use rusty_core as core;\n"
ASSET = """//! src/asset/mod.rs — the asset pipeline
use crate::asset::cache::Key;
use crate::core::paths::to_slash;
use crate::{core::fs, scene};
#[cfg(feature = "dev")]
fn f() { let _ = glam::Vec3::ZERO; let p = env!("CARGO_MANIFEST_DIR"); }
#[cfg(test)]
fn t() { crate::test_temp::dir(); }
"""


class Move(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = root = Path(tmp.name)
        files = {"Cargo.toml": CARGO, "src/lib.rs": LIB, "src/asset/mod.rs": ASSET,
                 "src/asset/cache.rs": "//! src/asset/cache.rs — keys\n", "src/scene/mod.rs": "",
                 "crates/core/src/lib.rs": ""}
        for name, text in files.items():
            (root / name).parent.mkdir(parents=True, exist_ok=True)
            (root / name).write_text(text)
        git = ["git", "-C", str(root)]
        subprocess.run([*git, "init", "-q"], check=True)
        subprocess.run([*git, "add", "."], check=True)
        self.hand = mover.move(root, "asset")

    def read(self, name: str) -> str:
        return (self.root / name).read_text()

    def test_the_files_move_and_their_paths_follow(self):
        self.assertFalse((self.root / "src/asset").exists())
        lib = self.read("crates/asset/src/lib.rs")
        self.assertTrue(lib.startswith("//! crates/asset/src/lib.rs — the asset pipeline"))
        self.assertIn("use crate::cache::Key;", lib)
        self.assertIn("use rusty_core::paths::to_slash;", lib)
        self.assertIn("rusty_test_support::temp::dir();", lib)
        self.assertTrue(self.read("crates/asset/src/cache.rs").startswith("//! crates/asset/src/cache.rs"))

    def test_the_manifest_names_what_the_code_names(self):
        cargo = self.read("crates/asset/Cargo.toml")
        self.assertIn('name = "rusty-asset"', cargo)
        self.assertIn("[lints]\nworkspace = true", cargo)
        self.assertIn("rusty-core = { workspace = true }\nglam = { workspace = true }", cargo)
        self.assertIn("[features]\ndev = []", cargo)
        self.assertNotIn("rustc-hash", cargo)

    def test_the_root_learns_the_crate(self):
        cargo = self.read("Cargo.toml")
        self.assertIn('rusty-asset = { path = "crates/asset" }', cargo)
        self.assertIn("# The moved modules.\nrusty-asset = { workspace = true }", cargo)
        self.assertIn('dev = ["rusty-asset/dev", "rusty-core/dev"', cargo)
        self.assertEqual(self.read("src/lib.rs"),
                         "pub mod scene;\n\npub use rusty_core as core;\npub use rusty_asset as asset;\n")

    def test_what_text_cannot_settle_is_listed(self):
        whys = "\n".join(self.hand)
        self.assertIn("crates/asset/src/lib.rs:4: a `crate::{…}` group", whys)
        self.assertIn("lib.rs:6: names the crate's folder", whys)

    def test_a_module_already_moved_is_refused(self):
        with self.assertRaises(SystemExit):
            mover.move(self.root, "asset")


if __name__ == "__main__":
    unittest.main()
