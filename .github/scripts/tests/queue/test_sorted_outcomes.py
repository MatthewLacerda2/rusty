#!/usr/bin/env python3
"""#1723's review: the take records a conflict's real outcome, after the check
and the push; a Markdown-only `main` that conflicts still reaches the resolver;
and one path git cannot rewrite in `diff3` leaves the others readable.

Over real repositories, as [`test_sorted_take`] is.
"""

from __future__ import annotations

import unittest

from merging.queue.conflict import record
from tests.queue.test_sorted_take import DOCS, RUST, Fixture


class Outcomes(Fixture, unittest.TestCase):
    def test_a_markdown_only_main_that_conflicts_is_resolved_not_skipped(self):
        self.branch({"docs/index.md": DOCS.replace("- [`Z`", "- [`Sfx`](Sfx.md)\n- [`Z`")})
        self.repo.commit({"docs/index.md": DOCS.replace("- [`Z`", "- [`Mesh`](Mesh.md)\n- [`Z`")}, "main")
        fresh, notes = self.advance()
        self.assertIsNotNone(fresh, notes)
        self.assertNotEqual(fresh, self.repo.head)
        self.assertEqual(self.repo.on_origin("b"), fresh)
        self.assertIn("- [`Mesh`](Mesh.md)\n- [`Sfx`](Sfx.md)\n", self.shown("docs/index.md"))
        self.assertEqual(record.read(self.log)[0]["ended"], record.RESOLVED)

    def test_a_resolved_merge_that_fails_the_check_is_recorded_handed_back(self):
        self.branch({"src/mods.rs": RUST.replace("mod m;\n", "mod m;\nmod q;\n")})
        self.repo.commit({"src/mods.rs": RUST.replace("mod m;\n", "mod m;\nmod p;\n")}, "main")
        fresh, notes = self.advance(check=lambda work: ["the check failed"])
        self.assertEqual((fresh, notes), (None, ["the check failed"]))
        self.assertEqual(self.repo.on_origin("b"), self.repo.head)
        [entry] = record.read(self.log)
        self.assertEqual((entry["ended"], entry["paths"]), (record.HANDED_BACK, {"src/mods.rs": True}))


class Deleted(Fixture, unittest.TestCase):
    BASE = {**Fixture.BASE, "src/gone.rs": "fn gone() {}\n"}

    def test_a_modify_delete_beside_a_resolvable_file_leaves_that_file_resolvable(self):
        self.branch({"src/mods.rs": RUST.replace("mod m;\n", "mod m;\nmod q;\n"), "src/gone.rs": "fn kept() {}\n"})
        self.repo.git("rm", "--quiet", "src/gone.rs")
        self.repo.commit({"src/mods.rs": RUST.replace("mod m;\n", "mod m;\nmod p;\n")}, "main")
        fresh, _ = self.advance()
        self.assertIsNone(fresh)
        [entry] = record.read(self.log)
        self.assertEqual((entry["ended"], entry["paths"]), (record.HANDED_BACK, {"src/gone.rs": False, "src/mods.rs": True}))


if __name__ == "__main__":
    unittest.main()
