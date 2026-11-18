#!/usr/bin/env python3
"""`.cargo/mutants.toml` skips exactly the serde default functions.

Their value is a design choice a test could only repeat, so cargo-mutants never
mutates them (`mutants_serde_defaults.py` says why). The list is generated, and
this fails when the config and the source disagree: a new default function not
skipped, or a skip left for one that moved or went.
"""

from __future__ import annotations

import re
import tempfile
import tomllib
import unittest
from pathlib import Path

from tests import SCRIPTS, load

found = load("mutants_serde_defaults.py")
CONFIG = SCRIPTS.parent.parent / ".cargo" / "mutants.toml"


def tree(files: dict[str, str]) -> Path:
    """A throwaway repository root holding `files`."""
    root = Path(tempfile.mkdtemp())
    for path, text in files.items():
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text(text)
    return root


class Scan(unittest.TestCase):
    def test_a_free_default_is_named_in_its_own_file(self) -> None:
        root = tree({"crates/c/src/a.rs": '#[serde(default = "half")]\nx: f32,\nfn half() -> f32 { 0.5 }\n'})
        self.assertEqual(found.defaults(root), {"crates/c/src/a.rs": ["half"]})

    def test_an_imported_default_is_named_where_it_is_defined(self) -> None:
        root = tree({
            "crates/c/src/mod.rs": 'use defaults::{one};\n#[serde(\n  default = "one",\n)]\nx: u32,\n',
            "crates/c/src/defaults.rs": "pub fn one() -> u32 { 1 }\n",
            "crates/c/src/other.rs": "fn one() -> u32 { 1 }\n",
        })
        self.assertEqual(found.defaults(root), {"crates/c/src/defaults.rs": ["one"]})

    def test_a_method_and_a_test_module_are_left_out(self) -> None:
        root = tree({
            "crates/c/src/a.rs": '#[serde(default = "Curve::flat")]\nx: Curve,\n',
            "crates/c/src/a_tests.rs": '#[serde(default = "on")]\nx: bool,\nfn on() -> bool { true }\n',
        })
        self.assertEqual(found.defaults(root), {})

    def test_an_ambiguous_default_says_how_to_name_it(self) -> None:
        root = tree({
            "crates/c/src/a.rs": '#[serde(default = "one")]\nx: u32,\n',
            "crates/c/src/b.rs": "fn one() -> u32 { 1 }\n",
            "crates/c/src/d.rs": "fn one() -> u32 { 1 }\n",
        })
        with self.assertRaisesRegex(SystemExit, "name it by its module path"):
            found.defaults(root)

    def test_a_line_matches_its_mutants_and_no_other(self) -> None:
        [line] = found.lines({"crates/c/src/a.rs": ["half", "one"]})
        match = re.compile(line).search
        self.assertTrue(match("crates/c/src/a.rs:3:5: replace half -> f32 with 0.0"))
        self.assertFalse(match("crates/c/src/a.rs:3:5: replace half_life -> f32 with 0.0"))
        self.assertFalse(match("crates/c/src/a.rs:3:5: replace Curve::half -> f32 with 0.0"))
        self.assertFalse(match("crates/c/src/b.rs:3:5: replace half -> f32 with 0.0"))


class Config(unittest.TestCase):
    def test_the_config_skips_exactly_the_serde_defaults(self) -> None:
        listed = tomllib.loads(CONFIG.read_text()).get("exclude_re", [])
        have = {line for line in listed if line.startswith(found.PREFIX)}
        want = set(found.lines(found.defaults()))
        self.assertEqual(have, want, "\n.cargo/mutants.toml's serde-default `exclude_re` lines are stale:"
                         " replace them with what `python3 .github/scripts/mutants_serde_defaults.py`"
                         " prints.")


if __name__ == "__main__":
    unittest.main()
