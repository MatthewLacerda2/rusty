#!/usr/bin/env python3
"""#979: a dev-dependency turning on a feature the normal build lacks is a split.

Canned `cargo tree -f '{p} [{f}]' --prefix none` output, so these run offline.
"""

from __future__ import annotations

import unittest

import feature_split

NORMAL = """\
ahash v0.8.12 [default,std]
getrandom v0.3.4 []
getrandom v0.3.4 [] (*)
rusty v0.1.0 (/home/user/rusty) [default,dev,editor]
"""


class Splits(unittest.TestCase):
    def test_flags_getrandom_std_turned_on_by_proptest(self):
        with_dev = NORMAL.replace("getrandom v0.3.4 []", "getrandom v0.3.4 [std]") + (
            "proptest v1.7.0 [default,std]\n")
        self.assertEqual(feature_split.splits(NORMAL, with_dev), {"getrandom v0.3.4": {"std"}})

    def test_passes_a_test_only_crate(self):
        with_dev = NORMAL + "proptest v1.7.0 [default,std]\nrand_core v0.9.3 [std]\n"
        self.assertEqual(feature_split.splits(NORMAL, with_dev), {})

    def test_two_versions_of_a_crate_are_two_crates(self):
        normal = NORMAL + "getrandom v0.2.17 []\n"
        with_dev = NORMAL + "getrandom v0.2.17 [std]\n"
        self.assertEqual(feature_split.splits(normal, with_dev), {"getrandom v0.2.17": {"std"}})

    def test_resolution_merges_repeated_lines_and_drops_the_path(self):
        crates = feature_split.resolution(NORMAL)
        self.assertEqual(crates["rusty v0.1.0"], {"default", "dev", "editor"})
        self.assertEqual(crates["getrandom v0.3.4"], set())


if __name__ == "__main__":
    unittest.main()
