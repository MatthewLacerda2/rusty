"""`make move-crate` at its edges: a submodule named like a module above, a re-aimed
string path, a later manifest table, and an include whose file did not move."""

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
image = "0.25"

[dependencies]
# The moved modules.
rusty-core = { workspace = true }
glam = { workspace = true }

[dev-dependencies]
glam = { workspace = true }
image = { workspace = true }
"""
LIB = "pub mod asset;\npub mod ui;\n\npub use rusty_core as core;\n"
TABLE = 'sim("core", &[]),\n    sim("asset", &["core"]),\n    sim("ui", &["asset", "core"]),\n'
ASSET = ("pub mod ui;\nuse crate::asset::ui::Panel;\n"
         'fn f() { let _ = glam::Vec3::ZERO; let _ = image::load; let p = "src/asset/x.png"; }\n')
OUTSIDE = ('const A: &str = include_str!("./a.wgsl");\nconst B: &str = include_str!("../x/../y.txt");\n'
           'const C: &str = include_str!("../asset/ui.rs");\n')


class Edges(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = root = Path(tmp.name)
        files = {"Cargo.toml": CARGO, "src/lib.rs": LIB, "src/asset/mod.rs": ASSET,
                 "src/asset/ui.rs": "pub struct Panel;\n", "src/ui/mod.rs": "",
                 "src/render/mod.rs": OUTSIDE, "crates/core/src/lib.rs": "",
                 "tools/lint/src/layers/table.rs": TABLE}
        for name, text in files.items():
            (root / name).parent.mkdir(parents=True, exist_ok=True)
            (root / name).write_text(text)
        git = ["git", "-C", str(root)]
        subprocess.run([*git, "init", "-q"], check=True)
        subprocess.run([*git, "add", "."], check=True)
        self.hand = "\n".join(mover.move(root, "asset"))

    def read(self, name: str) -> str:
        return (self.root / name).read_text()

    def test_its_own_submodule_named_like_a_module_above_is_not_reaching_up(self):
        self.assertIn("use crate::ui::Panel;", self.read("crates/asset/src/lib.rs"))
        self.assertNotIn("names `ui`", self.hand)

    def test_a_string_path_the_rewrite_re_aimed_is_listed(self):
        self.assertIn('"crates/asset/src/x.png"', self.read("crates/asset/src/lib.rs"))
        self.assertIn("crates/asset/src/lib.rs:3: a path from the checkout's root", self.hand)

    def test_a_later_table_keeps_its_lines(self):
        cargo = self.read("Cargo.toml")
        deps, dev = cargo.split("[dev-dependencies]")
        self.assertNotIn("glam", deps.split("[dependencies]")[1])
        self.assertIn("glam = { workspace = true }\nimage = { workspace = true }", dev)

    def test_an_include_whose_file_did_not_move_is_left_as_written(self):
        outside = self.read("src/render/mod.rs")
        self.assertIn('include_str!("./a.wgsl")', outside)
        self.assertIn('include_str!("../x/../y.txt")', outside)
        self.assertIn('include_str!("../../crates/asset/src/ui.rs")', outside)


if __name__ == "__main__":
    unittest.main()
