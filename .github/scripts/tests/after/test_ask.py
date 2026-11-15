"""The `Mutation:` line a coder writes, read as the queue reads it (#1934)."""

from __future__ import annotations

import unittest

from merging.after import ask
from tests import load

scope = load("mutants-scope.py")


class Requested(unittest.TestCase):
    def test_diff_and_globs_are_read(self) -> None:
        self.assertEqual(ask.requested("Closes #1\n\nMutation: diff\n"), "diff")
        self.assertEqual(ask.requested("Mutation: `src/physics/**` src/a.rs"), "src/physics/** src/a.rs")

    def test_a_comma_list_passes_through_for_the_scope_script_to_split(self) -> None:
        for value, passed in (("a/x.rs, b/y.rs", "a/x.rs, b/y.rs"), ("a/x.rs,b/y.rs", "a/x.rs,b/y.rs"),
                              ("`a/x.rs`, `b/y.rs`,", "a/x.rs , b/y.rs ,"), ("a.rs , b.rs", "a.rs , b.rs")):
            self.assertEqual(ask.requested(f"Mutation: {value}"), passed, value)
            self.assertEqual(scope.words(passed), [w for w in ("a/x.rs", "b/y.rs", "a.rs", "b.rs")
                                                   if w in passed], value)

    def test_a_brace_glob_keeps_its_commas(self) -> None:
        self.assertEqual(ask.requested("Mutation: src/{a,b}/**, c/d.rs"), "src/{a,b}/**, c/d.rs")

    def test_absent_or_empty_asks_for_nothing(self) -> None:
        for body in (None, "", "no line here", "Mutation:   ", "We said Mutation: diff inline."):
            self.assertIsNone(ask.requested(body), body)

    def test_an_unreadable_value_asks_for_nothing(self) -> None:
        for value in ("please mutate the physics", "src/a.rs; rm -rf /", "$(x)", "diff src/a.rs"):
            self.assertIsNone(ask.requested(f"Mutation: {value}"), value)

    def test_rust_is_diff_s_rule(self) -> None:
        self.assertTrue(ask.rust(["docs/x.md", "src/a.rs"]))
        self.assertFalse(ask.rust(["docs/x.md", ".github/scripts/x.py"]))
