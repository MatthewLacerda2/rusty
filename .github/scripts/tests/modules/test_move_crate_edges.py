"""`make move-crate` at its edges: a submodule named like a module above, a re-aimed
string path, a later manifest table, an include whose file did not move, a `lib.rs`
header that named its directory, another crate's folder named like the module's, and a visibility naming the module's root."""

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
         'fn f() { let _ = glam::Vec3::ZERO; let _ = image::load; let _ = crate::core::P; let p = "src/asset/x.png"; }\n')
OUTSIDE = ('const A: &str = include_str!("./a.wgsl");\nconst B: &str = include_str!("../x/../y.txt");\n'
           'const C: &str = include_str!("../asset/ui.rs");\n')


class Edges(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = root = Path(tmp.name)
        (root / "src/asset").mkdir(parents=True)
        (root / "src/asset/x.bin").write_bytes(b"\xff\xfe src/asset/")
        files = {"Cargo.toml": CARGO, "src/lib.rs": LIB, "src/asset/mod.rs": ASSET,
                 "src/asset/ui.rs": "pub struct Panel;\n", "src/ui/mod.rs": "",
                 "src/render/mod.rs": OUTSIDE, "crates/core/src/lib.rs": "",
                 "crates/core/Cargo.toml": '[package]\nname = "rusty-core"\n\n[features]\ndev = []\n',
                 "crates/core/src/asset/x.rs": "//! crates/core/src/asset/x.rs\n",
                 "docs/a.md": "see `src/asset/ui.rs`\n", "tools/lint/src/layers/table.rs": TABLE,
                 "src/asset/make.py": "python3 src/asset/make.py\n",
                 "Makefile": "a:\n\tcat $(CURDIR)/src/asset/a ../src/asset/b ./src/asset/c\n",
                 ".github/scripts/tests/mutants/test_scope.py": "+++ b/src/asset/b.rs\n"}
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

    def test_a_crate_with_no_feature_code_declares_none_beside_a_sibling_with_one(self):
        self.assertNotIn("[features]", self.read("crates/asset/Cargo.toml"))
        self.assertNotIn("rusty-asset/dev", self.read("Cargo.toml"))

    def test_an_include_whose_file_did_not_move_is_left_as_written(self):
        outside = self.read("src/render/mod.rs")
        self.assertIn('include_str!("./a.wgsl")', outside)
        self.assertIn('include_str!("../x/../y.txt")', outside)
        self.assertIn('include_str!("../../crates/asset/src/ui.rs")', outside)


    def test_a_file_naming_the_folder_is_listed_and_another_crates_like_named_folder_is_not(self):
        self.assertIn("docs/a.md:1: names `src/asset/`", self.hand)
        self.assertNotIn("crates/core/src/asset/x.rs", self.hand)

    def test_a_test_naming_a_module_below_takes_it_as_a_dev_dependency(self):
        hand = "\n".join(mover.reaching_up("x.rs", "crate::core::a();\ncrate::ui::b();\n", {"core", "ui"}, {"core"}))
        self.assertIn("x.rs:1: names `core`, which the row does not list: a test takes `rusty-core`", hand)
        self.assertIn("turning on its `dev` / `editor` features", hand)
        self.assertIn("x.rs:2: names `ui`, which the row does not list: a test reaching up", hand)

    def test_a_relative_path_outside_is_listed_and_a_fixture_is_not(self):
        self.assertIn("Makefile:2: names `src/asset/`", self.hand)
        self.assertNotIn("test_scope.py", self.hand)

    def test_any_other_text_file_in_the_crate_is_re_aimed_and_a_binary_one_kept(self):
        self.assertEqual(self.read("crates/asset/src/make.py"), "python3 crates/asset/src/make.py\n")
        self.assertEqual((self.root / "crates/asset/src/x.bin").read_bytes(), b"\xff\xfe src/asset/")


class Externs(unittest.TestCase):
    def test_a_name_a_file_declares_as_its_own_is_no_crate_there(self):
        for own in ("pub mod image;\npub use image::Look;\n", "use super::image;\nfn f() { image::look(); }\n",
                    "use crate::ui::{image, x};\nfn f() { image::look(); }\n", "use crate::ui::look as image;\n"):
            self.assertEqual(mover.externs([own, "fn g() { glam::Vec3::ZERO; }\n"], ["glam", "image"]), ["glam"])

    def test_another_file_naming_it_is_still_the_crate(self):
        files = ["pub mod image;\npub use image::Look;\n", "fn f() { image::load(); }\n"]
        self.assertEqual(mover.externs(files, ["image"]), ["image"])


class Visibility(unittest.TestCase):
    def test_a_path_restricted_to_the_module_root_names_the_crate(self):
        text, _ = mover.rewrite("pub(in crate::ecs) fn a() {}\npub(in crate::ecs::changes) fn b() {}\n", "ecs", set())
        self.assertEqual(text, "pub(crate) fn a() {}\npub(in crate::changes) fn b() {}\n")

    def test_one_naming_a_longer_module_is_left_alone(self):
        text, _ = mover.rewrite("pub(in crate::ecsx) fn a() {}\n", "ecs", set())
        self.assertEqual(text, "pub(in crate::ecsx) fn a() {}\n")


class LibHeader(unittest.TestCase):
    def test_a_header_naming_the_directory_names_lib_rs(self):
        for was, tail in (("//! src/asset — x", " — x"), ("//! crates/asset/src/ — x", " — x"),
                          ("#![deny(x)]\n//! crates/asset/src: x", ": x")):
            now = mover.lib_header(f"{was}\n//! src/asset y\n", "asset")
            self.assertIn(f"//! crates/asset/src/lib.rs{tail}\n//! src/asset y\n", now)

    def test_any_other_header_is_left_alone(self):
        for text in ("//! crates/asset/src/lib.rs — x", "//! the asset pipeline, src/asset", "//! src/assets — x"):
            self.assertEqual(mover.lib_header(text, "asset"), text)


if __name__ == "__main__":
    unittest.main()
