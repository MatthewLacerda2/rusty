"""The player build resolves no editor or dev crate (`player_tree.py`)."""

from __future__ import annotations

import unittest

from tests import load

tree = load("player_tree.py")

PLAYER = """rusty v0.1.0 (/r)
rusty-core v0.1.0 (/r/crates/core)
wgpu v30.0.1
scorsese-zimmer v0.1.0 (https://github.com/x)
"""


class PlayerTree(unittest.TestCase):
    def test_a_clean_player_passes(self):
        self.assertEqual(tree.banned(tree.crates(PLAYER)), [])

    def test_egui_the_editor_and_the_dev_layer_are_flagged(self):
        leaked = PLAYER + "egui-winit v0.36.2\nrusty-dev v0.1.0 (/r/crates/dev)\nrusty-editor v0.1.0 (*)\n"
        found = tree.banned(tree.crates(leaked))
        self.assertEqual([f.split(":")[0] for f in found], ["egui-winit", "rusty-dev", "rusty-editor"])

    def test_a_name_only_sharing_a_prefix_with_a_banned_one_passes(self):
        self.assertEqual(tree.banned({"rusty-editorial", "rusty-device"}), [])


if __name__ == "__main__":
    unittest.main()
