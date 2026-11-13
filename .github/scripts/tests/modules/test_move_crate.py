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
use crate::asset::{cache, Thing};
use crate::core::paths::to_slash;
use crate::{core::fs, scene};
#[cfg(feature = "dev")]
fn f() { let _ = glam::Vec3::ZERO; let p = env!("CARGO_MANIFEST_DIR"); }
#[cfg(test)]
fn t() { crate::test_temp::dir(); }
"""
TABLE = 'sim("core", &[]),\n    sim(\n        "asset",\n        &["core"],\n    ),\n    sim("scene", &["asset", "core"]),\n'
INC = 'fn d() { let _ = std::path::Path::new("tests/fixtures/a.fbx"); }\nconst F: &str = include_str!("../../../tests/f.fbx");\nconst O: &str = include_str!("../cache.rs");\n'
UP = "use crate::logging;\nfn bake() { crate::scene::bake(); }\n"


class Move(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = root = Path(tmp.name)
        files = {"Cargo.toml": CARGO, "src/lib.rs": LIB, "src/asset/mod.rs": ASSET,
                 "src/asset/cache.rs": "//! src/asset/cache.rs — keys\n", "src/scene/mod.rs": "", "src/logging/mod.rs": "",
                 "src/asset/up_tests.rs": UP, "crates/core/src/lib.rs": "",
                 "tools/lint/src/layers/table.rs": TABLE, "src/asset/deep/inc_tests.rs": INC,
                 "src/dev/mp3.rs": 'const A: &[u8] = include_bytes!("../asset/cache.rs");\n',
                 "proptest-regressions/asset/deep/inc_tests.txt": "cc 1\n", "proptest-regressions/scene/a.txt": "cc 2\n"}
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
        self.assertIn("crates/asset/src/lib.rs:5: a `crate::{…}` group", whys)
        self.assertNotIn("lib.rs:3: a `crate::{…}` group", whys)
        self.assertIn("lib.rs:7: names the crate's folder", whys)

    def test_a_module_above_it_is_listed_not_made_a_dependency(self):
        self.assertEqual(self.read("crates/asset/src/up_tests.rs"), UP)
        self.assertNotIn("rusty-scene", self.read("crates/asset/Cargo.toml"))
        whys = "\n".join(self.hand)
        self.assertIn("crates/asset/src/up_tests.rs:1: names `logging`, which the row does not list", whys)
        self.assertIn("crates/asset/src/up_tests.rs:2: names `scene`, which the row does not list", whys)

    def test_a_relative_include_still_names_its_file(self):
        inc = self.read("crates/asset/src/deep/inc_tests.rs")
        self.assertIn('include_str!("../../../../tests/f.fbx")', inc)
        whys = "\n".join(self.hand)
        self.assertIn("inc_tests.rs:1: a path from the checkout's root read from the working directory", whys)
        self.assertNotIn("inc_tests.rs:2: a path from the checkout's root", whys)
        self.assertIn('include_str!("../cache.rs")', inc, "a file inside the crate moved with it")
        self.assertIn('include_bytes!("../../crates/asset/src/cache.rs")', self.read("src/dev/mp3.rs"))

    def test_its_proptest_seeds_move_beside_the_crate_and_no_others(self):
        self.assertEqual(self.read("crates/asset/proptest-regressions/deep/inc_tests.txt"), "cc 1\n")
        self.assertFalse((self.root / "proptest-regressions/asset").exists())
        self.assertEqual(self.read("proptest-regressions/scene/a.txt"), "cc 2\n")

    def test_a_crate_only_the_moved_code_names_leaves_the_root(self):
        cargo = self.read("Cargo.toml").split("[dependencies]")[1]
        self.assertNotIn("glam", cargo)
        self.assertIn("glam left the root's [dependencies]", "\n".join(self.hand))

    def test_a_module_with_no_row_is_refused(self):
        (self.root / "src/orphan").mkdir()
        with self.assertRaises(SystemExit):
            mover.move(self.root, "orphan")

    def test_a_module_already_moved_is_refused(self):
        with self.assertRaises(SystemExit):
            mover.move(self.root, "asset")


if __name__ == "__main__":
    unittest.main()
