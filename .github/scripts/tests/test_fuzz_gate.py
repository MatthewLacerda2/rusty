#!/usr/bin/env python3
"""#2200: the derived fuzz lock keeps the root lock's versions, and the local
`fuzz-check` asks ci.yml's `fuzz` filter whether to build."""

from __future__ import annotations

import unittest

import fuzz_gate


def lock(*pins: tuple[str, str]) -> str:
    return "version = 4\n\n" + "".join(f'[[package]]\nname = "{n}"\nversion = "{v}"\n\n' for n, v in pins)


ROOT = lock(("serde", "1.0.2"), ("bitflags", "1.3.2"), ("bitflags", "2.9.0"), ("rusty-scene", "0.1.0"))


class Drift(unittest.TestCase):
    def test_a_shared_package_the_resolve_moved_is_flagged(self):
        self.assertEqual(fuzz_gate.drift(ROOT, lock(("serde", "1.0.1"))), ["serde 1.0.1 (root: 1.0.2)"])

    def test_a_pruned_subset_and_fuzz_only_crates_pass(self):
        fuzz = lock(("bitflags", "2.9.0"), ("rusty-scene", "0.1.0"), ("libfuzzer-sys", "0.4.10"))
        self.assertEqual(fuzz_gate.drift(ROOT, fuzz), [])


class Filter(unittest.TestCase):
    def test_today_the_filter_is_found(self):
        globs = fuzz_gate.filter_globs((fuzz_gate.ROOT / ".github/workflows/ci.yml").read_text())
        self.assertIn("fuzz/**", globs)
        self.assertIn("crates/*/src/lib.rs", globs)

    def test_only_the_named_filter_is_read(self):
        yml = "filters: |\n  code:\n    - '**'\n  fuzz:\n    # why\n    - 'fuzz/**'\n    - 'src/lib.rs'\nnext: 1\n"
        self.assertEqual(fuzz_gate.filter_globs(yml), ["fuzz/**", "src/lib.rs"])
        self.assertEqual(fuzz_gate.filter_globs("code:\n  - '**'\n"), [])

    def test_a_star_stays_in_one_directory(self):
        globs = ["fuzz/**", "crates/*/src/lib.rs", "Cargo.toml"]
        paths = ["fuzz/fuzz_targets/a.rs", "crates/edit/src/lib.rs", "crates/edit/src/x/lib.rs",
                 "Cargo.toml", "crates/edit/Cargo.toml", "docs/testing.md"]
        self.assertEqual(fuzz_gate.touched(paths, globs), ["fuzz/fuzz_targets/a.rs", "crates/edit/src/lib.rs", "Cargo.toml"])


if __name__ == "__main__":
    unittest.main()
