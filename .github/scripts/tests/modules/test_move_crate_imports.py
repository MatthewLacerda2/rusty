"""`make move-crate`'s rewrite of the sibling forms a `crate::<dep>::` scan misses: a bare
`use crate::<dep>;`, a macro's `$crate::<dep>::`, and a crate a comment names."""

from __future__ import annotations

import unittest

from tests import load

mover = load("move_crate.py")


class BareImport(unittest.TestCase):
    def test_a_bare_sibling_import_goes_and_its_paths_name_the_crate(self):
        text, named = mover.rewrite("use crate::asset;\nfn f() { asset::load(); }\n", "scene", {"asset"})
        self.assertEqual(text, "fn f() { rusty_asset::load(); }\n")
        self.assertEqual(named, {"asset"})

    def test_a_self_in_a_group_goes_and_its_paths_name_the_crate(self):
        text, _ = mover.rewrite("use crate::asset::{self, A};\nfn f() { asset::load(); }\n", "scene", {"asset"})
        self.assertEqual(text, "use rusty_asset::{A};\nfn f() { rusty_asset::load(); }\n")

    def test_a_path_through_another_module_is_left_alone(self):
        text, _ = mover.rewrite("use crate::asset;\nfn f() { asset::a(); x::asset::b(); }\n", "scene", {"asset"})
        self.assertIn("x::asset::b()", text)

    def test_a_file_without_the_import_keeps_its_own_name(self):
        text, _ = mover.rewrite("mod asset;\nfn f() { asset::a(); }\n", "scene", {"asset"})
        self.assertEqual(text, "mod asset;\nfn f() { asset::a(); }\n")


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

    def test_one_the_code_names_still_is(self):
        self.assertEqual(mover.externs("// note\nfn f() { wgpu::x(); }\n", ["wgpu"]), ["wgpu"])


if __name__ == "__main__":
    unittest.main()
