"""`make move-crate`'s rewrite of the sibling forms a `crate::<dep>::` scan misses: a bare
`use crate::<dep>;` (an attribute above it, a re-export, a rename), a macro's `$crate::<dep>::`, and a crate only a comment or a string names."""

from __future__ import annotations

import unittest

from tests import load

mover = load("move_crate.py")
imports = load("crate_imports.py")


class BareImport(unittest.TestCase):
    def test_a_bare_sibling_import_goes_and_its_paths_name_the_crate(self):
        text, named = mover.rewrite("use crate::asset;\nfn f() { asset::load(); }\n", "scene", {"asset"})
        self.assertEqual(text, "fn f() { rusty_asset::load(); }\n")
        self.assertEqual(named, {"asset"})

    def test_a_self_in_a_group_goes_and_its_paths_name_the_crate(self):
        text, _ = mover.rewrite("use crate::asset::{self, A};\nfn f() { asset::load(); }\n", "scene", {"asset"})
        self.assertEqual(text, "use rusty_asset::{A};\nfn f() { rusty_asset::load(); }\n")

    def test_a_lone_self_goes_whole(self):
        text, named = mover.rewrite("#[cfg(test)]\nuse crate::asset::{self};\nfn f() { asset::a(); }\n", "scene", {"asset"})
        self.assertEqual(text, "fn f() { rusty_asset::a(); }\n")
        self.assertEqual(named, {"asset"})

    def test_a_path_through_another_module_is_left_alone(self):
        text, _ = mover.rewrite("use crate::asset;\nfn f() { asset::a(); x::asset::b(); }\n", "scene", {"asset"})
        self.assertIn("x::asset::b()", text)

    def test_a_file_without_the_import_keeps_its_own_name(self):
        text, _ = mover.rewrite("mod asset;\nfn f() { asset::a(); }\n", "scene", {"asset"})
        self.assertEqual(text, "mod asset;\nfn f() { asset::a(); }\n")


    def test_an_attribute_above_the_removed_import_goes_with_it(self):
        text, _ = mover.rewrite("#[cfg(test)]\nuse crate::asset;\nfn f() { asset::a(); }\n", "scene", {"asset"})
        self.assertEqual(text, "fn f() { rusty_asset::a(); }\n")

    def test_a_bare_re_export_keeps_its_name(self):
        text, named = mover.rewrite("pub use crate::asset;\n", "scene", {"asset"})
        self.assertEqual(text, "pub use rusty_asset as asset;\n")
        self.assertEqual(named, {"asset"})

    def test_a_renamed_import_names_the_crate_under_its_name(self):
        text, named = mover.rewrite("use crate::asset as a;\nfn f() { a::x(); }\n", "scene", {"asset"})
        self.assertEqual(text, "use rusty_asset as a;\nfn f() { a::x(); }\n")
        self.assertEqual(named, {"asset"})

    def test_a_re_exported_self_keeps_its_name(self):
        text, _ = mover.rewrite("pub use crate::asset::{self, A};\n", "scene", {"asset"})
        self.assertEqual(text, "pub use rusty_asset::{self as asset, A};\n")
        text, _ = mover.rewrite("pub(crate) use crate::asset::{self};\n", "scene", {"asset"})
        self.assertEqual(text, "pub(crate) use rusty_asset::{self as asset};\n")


class Ambiguous(unittest.TestCase):
    """A bare `<dep>::` in a `use` group under another root, or in a string, may not be the sibling."""

    FORMS = (
        "use crate::asset;\nuse x::{asset::A};\nfn f() { asset::a(); }\n",
        "use crate::asset::{self, B};\nuse x::{A, asset::C};\nfn f() { asset::a(); }\n",
        'use crate::asset;\nfn f() { asset::a(); let s = "asset::b"; }\n',
    )

    def test_the_file_is_left_alone_and_listed(self):
        for form in self.FORMS:
            text, _ = mover.rewrite(form, "scene", {"asset"})
            self.assertEqual(text, form.replace("crate::asset", "rusty_asset").replace("use rusty_asset;", "use crate::asset;"), form)
            hand = imports.unsettled("x.rs", imports.siblings(form, {"asset"})[0], {"asset"})
            self.assertTrue(hand and all("inside a `use` group or a string" in h for h in hand), f"{form}: {hand}")

    def test_a_block_or_an_argument_after_a_brace_or_comma_is_still_rewritten(self):
        text, _ = mover.rewrite("use crate::asset;\nfn f() {asset::a(); g(1, asset::b()); }\n", "scene", {"asset"})
        self.assertEqual(text, "fn f() {rusty_asset::a(); g(1, rusty_asset::b()); }\n")


class MacroCrate(unittest.TestCase):
    def test_a_macros_sibling_path_names_the_crate_from_the_root(self):
        text, named = mover.rewrite("fn r(w: &$crate::ecs::World) {}\n", "scene", {"ecs"})
        self.assertEqual(text, "fn r(w: &::rusty_ecs::World) {}\n")
        self.assertEqual(named, {"ecs"})

    def test_its_own_path_names_the_crate_itself(self):
        text, _ = mover.rewrite("$crate::scene::authoring::F\n", "scene", {"ecs"})
        self.assertEqual(text, "$crate::authoring::F\n")


class Comments(unittest.TestCase):
    def test_a_crate_only_a_comment_names_is_no_dependency(self):
        self.assertEqual(mover.externs("/// `wgpu::Limits::default()`\nfn f() {}\n", ["wgpu"]), [])

    def test_a_trailing_comment_a_block_comment_or_a_string_is_no_dependency(self):
        code = 'fn f() {} // wgpu::x\n/* a\n wgpu::Limits */\nconst S: &str = "wgpu::y";\n'
        self.assertEqual(mover.externs(code, ["wgpu"]), [])
        for raw in ('r"wgpu::a"', 'r#"wgpu::"a"#', 'br"wgpu::b"'):
            self.assertEqual(mover.externs(f"fn f() {{ let s = {raw}; }}\n", ["wgpu"]), [])
            self.assertEqual(mover.externs(f"fn f() {{ let s = {raw}; wgpu::x(); }}\n", ["wgpu"]), ["wgpu"])

    def test_one_the_code_names_still_is(self):
        self.assertEqual(mover.externs("// note\nfn f() { wgpu::x(); }\n", ["wgpu"]), ["wgpu"])


if __name__ == "__main__":
    unittest.main()
