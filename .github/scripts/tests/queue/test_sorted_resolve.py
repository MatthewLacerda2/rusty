#!/usr/bin/env python3
"""#1723: which conflicted files the sorted-list rule resolves, and how.

Pure: [`resolve.resolved`] over `diff3` texts as git writes them. The take and
the watch drive the same function over real merges (`test_sorted_take`, `watch.test_sorted_pretake`).
"""

from __future__ import annotations

import unittest
from pathlib import Path

from merging.queue.conflict import lists, resolve

REPO = Path(__file__).resolve().parents[4]


def conflict(ours: str, base: str, theirs: str, above: str = "// sorted: names only\nmod a;\n", below: str = "mod z;\n") -> str:
    return f"{above}<<<<<<< HEAD\n{ours}||||||| base\n{base}=======\n{theirs}>>>>>>> origin/main\n{below}"


class Resolves(unittest.TestCase):
    def test_two_entries_added_in_one_rust_list_are_both_kept_in_order(self):
        text = conflict("mod c;\n", "", "mod b;\n")
        self.assertEqual(resolve.resolved(text), "// sorted: names only\nmod a;\nmod b;\nmod c;\nmod z;\n")

    def test_a_markdown_list_too(self):
        above = "<!-- sorted: an index -->\n- [`A`](A.md)\n"
        text = conflict("- [`Sfx`](Sfx.md)\n", "", "- [`Mesh`](Mesh.md)\n", above, "- [`Time`](Time.md)\n")
        self.assertEqual(resolve.resolved(text), above + "- [`Mesh`](Mesh.md)\n- [`Sfx`](Sfx.md)\n- [`Time`](Time.md)\n")

    def test_the_same_entry_added_by_both_is_kept_once(self):
        self.assertEqual(resolve.resolved(conflict("mod b;\nmod c;\n", "", "mod b;\n")).count("mod b;"), 1)

    def test_a_multi_line_entry_moves_with_its_continuation(self):
        text = conflict("    gamma,\n    zeta(\n        1),\n", "    gamma,\n", "    delta(\n        2),\n    gamma,\n", "    // sorted: by name\n    beta,\n", "")
        self.assertEqual(resolve.resolved(text), "    // sorted: by name\n    beta,\n    delta(\n        2),\n    gamma,\n    zeta(\n        1),\n")


class HandsBack(unittest.TestCase):
    def test_a_conflict_in_a_file_with_no_marker(self):
        self.assertIsNone(resolve.resolved(conflict("mod c;\n", "", "mod b;\n", above="mod a;\n")))

    def test_one_hunk_in_a_list_and_one_outside_it(self):
        inside = conflict("mod c;\n", "", "mod b;\n", below="mod z;\n\n")
        outside = "<<<<<<< HEAD\nfn one() {}\n||||||| base\nfn zero() {}\n=======\nfn two() {}\n>>>>>>> origin/main\n"
        self.assertIsNone(resolve.resolved(inside + outside))

    def test_one_side_removing_or_editing_an_entry(self):
        self.assertIsNone(resolve.resolved(conflict("mod c;\n", "mod b;\n", "mod b;\nmod d;\n")))
        self.assertIsNone(resolve.resolved(conflict("mod b2;\nmod c;\n", "mod b;\n", "mod b;\nmod d;\n")))

    def test_the_same_key_with_different_text(self):
        self.assertIsNone(resolve.resolved(conflict("pub mod b;\n", "", "mod b;\n")))
        self.assertIsNone(resolve.resolved(conflict("mod B;\n", "", "mod b;\n")))

    def test_a_comment_or_attribute_line_in_the_hunk(self):
        self.assertIsNone(resolve.resolved(conflict("#[cfg(test)]\nmod c;\n", "", "mod b;\n")))
        self.assertIsNone(resolve.resolved(conflict("// c\nmod c;\n", "", "mod b;\n")))

    def test_a_comment_or_attribute_line_directly_above_the_hunk(self):
        # `#[cfg(test)]` would land on whichever entry sorts first, not on `mod p;`.
        self.assertIsNone(resolve.resolved(conflict("mod p;\n", "", "mod b;\n", above="// sorted: x\nmod a;\n#[cfg(test)]\n")))
        self.assertIsNone(resolve.resolved(conflict("mod p;\n", "", "mod b;\n", above="// sorted: x\nmod a;\n// p\n")))
        # The list's own marker directly above is no annotation.
        self.assertIsNotNone(resolve.resolved(conflict("mod c;\n", "", "mod b;\n", above="// sorted: x\n")))

    def test_a_hunk_with_no_base_section_or_broken_markers(self):
        self.assertIsNone(resolve.resolved("// sorted: x\nmod a;\n<<<<<<< HEAD\nmod c;\n=======\nmod b;\n>>>>>>> m\n"))
        self.assertIsNone(resolve.resolved("// sorted: x\nmod a;\n<<<<<<< HEAD\nmod c;\n||||||| b\n=======\nmod b;\n"))
        self.assertIsNone(resolve.resolved("// sorted: x\nmod a;\nmod b;\n"))

    def test_cargo_lock_is_never_resolved_here(self):
        self.assertIsNone(resolve.decide("Cargo.lock", conflict("mod c;\n", "", "mod b;\n")))
        self.assertIsNotNone(resolve.decide("src/x.rs", conflict("mod c;\n", "", "mod b;\n")))

    def test_a_hunk_that_runs_past_the_list_end(self):
        self.assertIsNone(resolve.resolved(conflict("mod c;\n\nfn f() {}\n", "", "mod b;\n")))


class Mirror(unittest.TestCase):
    def test_reads_the_shared_key_table_as_the_lint_rule_does(self):
        table = REPO / "tools/lint/src/rules/sorted_keys.tsv"
        rows = [r for r in table.read_text().splitlines() if r and not r.startswith("# ")]
        self.assertGreater(len(rows), 10)
        for row in rows:
            line, want = row.split("\t")
            self.assertEqual(lists.spelled(line), want, line)

    def test_the_system_schedule_carries_no_marker_so_it_is_never_resolved(self):
        # Its order is the frame's system schedule (#1723's decision).
        lines = (REPO / "crates/app/src/registry.rs").read_text().splitlines()
        self.assertFalse([line for line in lines if lists.reason(line) is not None])


if __name__ == "__main__":
    unittest.main()
