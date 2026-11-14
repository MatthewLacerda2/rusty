"""`make move-crate`'s by-hand list of the sibling imports its plain rules leave alone, and the
rows below a module that a test may take as a dev-dependency."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from tests import load

mover = load("move_crate.py")
imports = load("crate_imports.py")

#: Each form binds `asset` by a `self` the plain rules do not settle: it is listed, never rewritten.
ODD = (
    "use crate::asset::{A, self};\n",                      # `self` not first
    "use crate::asset::{self, loader::{Opts, B}};\n",      # a nested group beside it
    "use crate::asset::{self, /* why */ A};\n",            # a comment in the group
    "use crate::asset::{self,};\n",                        # a trailing comma
    "use crate::asset::{\n    self,\n    A,\n};\n",        # over several lines
    "pub use crate::asset::{A, self};\n",                  # a re-export, `self` not first
)


class Unsettled(unittest.TestCase):
    def test_each_odd_form_is_listed_not_rewritten(self):
        for form in ODD:
            text, _ = mover.rewrite(form + "fn f() { asset::a(); }\n", "scene", {"asset"})
            self.assertEqual(text, form.replace("crate::asset", "rusty_asset") + "fn f() { asset::a(); }\n", form)
            hand = imports.unsettled("x.rs", text, {"asset"})
            self.assertEqual(len(hand), 1, f"{form}: {hand}")
            self.assertIn("x.rs:1: a `rusty_asset::{…}` group binds `self`", hand[0])

    def test_a_nested_self_binds_its_own_name_and_is_settled(self):
        for form in ("use crate::asset::{loader::{self, Opts}, A};\n", "pub use crate::asset::{loader::{self, O}, A};\n"):
            text, _ = mover.rewrite(form, "scene", {"asset"})
            self.assertEqual(text, form.replace("crate::asset", "rusty_asset"))
            self.assertEqual(imports.unsettled("x.rs", text, {"asset"}), [])

    def test_a_plain_form_leaves_nothing_listed(self):
        for form in ("use crate::asset;\n", "use crate::asset::{self, A};\n", "pub use crate::asset::{self, A};\n",
                     "use crate::asset::{\n    A,\n    B,\n};\n", "use crate::asset as a;\n"):
            text, _ = mover.rewrite(form + "fn f() { asset::a(); }\n", "scene", {"asset"})
            self.assertEqual(imports.unsettled("x.rs", text, {"asset"}), [], form)

    def test_a_crate_dep_the_rules_left_is_listed(self):
        text, _ = mover.rewrite("use crate::asset; // why\n", "scene", {"asset"})
        self.assertIn("`crate::asset` the rewrite did not settle", "\n".join(imports.unsettled("x.rs", text, {"asset"})))

    def test_an_own_submodule_named_like_a_sibling_is_not_listed(self):
        before = "use crate::api::physics::Ray;\nfn f() { crate::api::physics::cast(); }\n"
        self.assertEqual(mover.unsettled("x.rs", before, {"physics"}), [])
        self.assertEqual(len(mover.unsettled("x.rs", "use crate::physics; // why\n", {"physics"})), 1)


class CfgTest(unittest.TestCase):
    def test_a_cfg_test_switch_in_live_code_is_listed(self):
        text = "fn f() -> bool {\n    cfg!(test) || x()\n}\nlet k = cfg!(any(test, unix));\n"
        hand = mover.by_hand("crates/a/src/lib.rs", text, text)
        self.assertEqual([h.split(": ", 1)[0] for h in hand], ["crates/a/src/lib.rs:2", "crates/a/src/lib.rs:4"])
        self.assertIn("cfg-test-switch", hand[0])

    def test_a_test_file_an_allowed_one_and_a_comment_are_not(self):
        text = "// lint: allow(cfg-test-switch) why\nlet t = cfg!(test);\n// cfg!(test)\nlet d = cfg!(debug_assertions);\n"
        self.assertEqual(mover.by_hand("crates/a/src/lib.rs", text, text), [])
        self.assertEqual(mover.by_hand("crates/a/src/a_tests.rs", "let t = cfg!(test);\n", ""), [])


TABLE = 'sim("core", &[]),\n    sim("ui", &["core"]),\n    sim("script", &["core", "ui"]),\n'


class RowsBelow(unittest.TestCase):
    def test_the_rows_before_the_module_are_below_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / mover.TABLE).parent.mkdir(parents=True)
            (root / mover.TABLE).write_text(TABLE)
            self.assertEqual(mover.rows_below(root, "script"), {"core", "ui"})
            self.assertEqual(mover.rows_below(root, "ui"), {"core"})


if __name__ == "__main__":
    unittest.main()
