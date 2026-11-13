"""`make move-crate` keeps what a feature gates gated, and test-only crates out of the build."""

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
egui = { version = "0.36" }
glam = { version = "0.33" }
proptest = "1"
objc2 = "0.6"

[features]
dev = ["rusty-core/dev", "dep:scorsese-core"]
editor = ["dep:egui"]

[dependencies]
# The moved modules.
rusty-core = { workspace = true }
egui = { workspace = true, optional = true }
glam = { workspace = true }

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
"""
LIB = '#[cfg(feature = "editor")]\npub mod shell;\npub mod render;\n#[cfg(feature = "dev")]\npub mod dev;\n\npub use rusty_core as core;\n'
SHELL = """//! src/shell/mod.rs — the shell
use crate::core::paths;
use crate::render::scene::Draw;
use crate::shell::scene::Window;
#[cfg(feature = "dev")]
use crate::dev::Harness;
#[cfg(feature = "editor")]
fn ui(c: &egui::Context) { let _ = glam::Vec3::ZERO; let _ = objc2::ffi::nil; }
#[cfg(test)]
mod t {
    fn f() { let _ = proptest::bool::ANY; }
}
"""
FILES = {
    "Cargo.toml": CARGO, "src/lib.rs": LIB, "src/shell/mod.rs": SHELL,
    "src/shell/scene.rs": "", "src/shell/tests.rs": "fn g() { proptest::prop_assert!(true); }\n",
    "src/render/mod.rs": "", "src/dev/mod.rs": "", "src/render/scene.rs": "",
    "src/render/read_tests.rs": 'const F: &str = "src/shell/fixtures/a.png";\n',
    "tools/lint/src/layers/table.rs": 'platform("shell", &["core", "dev", "render"]),\n',
    "crates/core/src/lib.rs": "", "crates/core/Cargo.toml": '[package]\nname = "rusty-core"\n\n[features]\ndev = []\n',
}


class Gated(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = root = Path(tmp.name)
        for name, text in FILES.items():
            (root / name).parent.mkdir(parents=True, exist_ok=True)
            (root / name).write_text(text)
        subprocess.run(["git", "-C", str(root), "init", "-q"], check=True)
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)
        self.hand = mover.move(root, "shell")
        self.cargo = (root / "crates/shell/Cargo.toml").read_text()

    def test_an_optional_dependency_stays_optional_behind_its_feature(self):
        self.assertIn("egui = { workspace = true, optional = true }", self.cargo)
        self.assertIn('editor = ["dep:egui"]', self.cargo)
        self.assertIn('dev = ["dep:rusty-dev", "rusty-core/dev"]', self.cargo)
        self.assertIn("rusty-dev = { workspace = true, optional = true }", self.cargo)
        self.assertIn("glam = { workspace = true }\n", self.cargo)

    def test_a_target_only_dependency_keeps_its_table(self):
        self.assertIn("[target.'cfg(target_os = \"macos\")'.dependencies]\nobjc2 = { workspace = true }", self.cargo)

    def test_a_crate_only_tests_name_is_a_dev_dependency(self):
        deps = self.cargo.split("[dev-dependencies]")
        self.assertNotIn("proptest", deps[0])
        self.assertIn("proptest = { workspace = true }", deps[1])

    def test_a_gated_module_stays_gated_in_the_root(self):
        root = (self.root / "Cargo.toml").read_text()
        self.assertIn("rusty-shell = { workspace = true, optional = true }", root)
        self.assertIn('editor = ["dep:rusty-shell", "rusty-shell/editor", "dep:egui"]', root)
        self.assertIn('dev = ["rusty-shell?/dev", "rusty-core/dev"', root)
        lib = (self.root / "src/lib.rs").read_text()
        self.assertIn('#[cfg(feature = "editor")]\npub use rusty_shell as shell;\n', lib)
        self.assertNotIn("pub mod shell;", lib)

    def test_a_submodule_named_like_a_module_stays_the_crates_own(self):
        lib = (self.root / "crates/shell/src/lib.rs").read_text()
        self.assertIn("use rusty_render::scene::Draw;", lib)
        self.assertIn("use crate::scene::Window;", lib)
        self.assertNotIn("rusty_scene", lib)

    def test_a_file_outside_naming_the_old_folder_is_listed(self):
        self.assertTrue(any(h.startswith("src/render/read_tests.rs:1: names `src/shell/`") for h in self.hand),
                        self.hand)


class Unsettled(unittest.TestCase):
    def test_a_gate_on_anything_but_one_feature_is_listed(self):
        hand: list[str] = []
        lib = '#[cfg(all(feature = "dev", unix))]\npub mod dev;\n'
        self.assertIsNone(mover.root_gate(lib, "dev", hand))
        self.assertIn("gated by `all(", hand[0])

    def test_an_ungated_module_has_no_gate(self):
        self.assertIsNone(mover.root_gate("pub mod dev;\n", "dev", []))

    def test_live_code_drops_test_items(self):
        code = "fn a() {}\n#[cfg(test)]\nuse proptest::x;\n#[cfg(test)]\nmod t {\n  fn b() {}\n}\nfn c() {}\n"
        self.assertEqual(mover.live(code), "fn a() {}\nfn c() {}")


if __name__ == "__main__":
    unittest.main()
