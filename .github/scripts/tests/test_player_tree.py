"""The player build resolves no editor, dev or HTTP crate (`player_tree.py`)."""

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

    def test_egui_the_dev_layer_and_the_http_client_are_flagged(self):
        leaked = PLAYER + "egui-winit v0.36.2\nrusty-dev v0.1.0 (/r/crates/dev)\nureq v3.0.0 (*)\n"
        found = tree.banned(tree.crates(leaked))
        self.assertEqual([f.split(":")[0] for f in found], ["egui-winit", "rusty-dev", "ureq"])

    def test_a_name_only_sharing_a_prefix_with_a_banned_one_passes(self):
        self.assertEqual(tree.banned({"rusty-editorial", "ureq-proto", "rusty-device"}), [])


if __name__ == "__main__":
    unittest.main()
