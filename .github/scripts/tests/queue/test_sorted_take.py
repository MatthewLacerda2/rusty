#!/usr/bin/env python3
"""#1723: the take resolves a conflict inside a `sorted` list and pushes it like a
clean merge; anything else is handed back whole, and every conflict is recorded.

Over real repositories (a bare `origin` and a clone, [`test_update_by_merge.Origin`]),
so the merge, the `diff3` rewrite, the commit and the leased push are git's own.
"""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from merging.queue import update
from merging.queue.conflict import record
from tests.queue.test_markdown_advance import WHO
from tests.queue.test_update_by_merge import Origin

RUST = "// sorted: names only\nmod a;\nmod m;\nmod z;\n"
DOCS = "<!-- sorted: an index -->\n- [`A`](A.md)\n- [`Z`](Z.md)\n"


class Taking(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        env = mock.patch.dict(os.environ, WHO)
        env.start()
        self.addCleanup(env.stop)
        self.repo = Origin(tmp.name)
        self.log = Path(tmp.name, "conflicts.jsonl")
        # Both lists on `main`, then `b` rebuilt on top of them.
        self.repo.commit({"src/mods.rs": RUST, "docs/index.md": DOCS}, "main")
        self.repo.git("checkout", "--quiet", "-B", "b", "main")
        self.repo.git("push", "--quiet", "--force", "origin", "main:refs/heads/b")  # a fixture's own branch

    def branch(self, files: dict[str, str]) -> None:
        self.repo.head = self.repo.commit(files, "b")
        self.repo.git("checkout", "--quiet", "main")

    def advance(self):
        log = lambda main, paths, ended: record.note(self.log, 7, self.repo.head, main, paths, ended)
        return update.advance("b", self.repo.head, self.repo.root, push=True, log=log)

    def shown(self, path: str) -> str:
        return self.repo.git("show", f"origin/b:{path}")

    def test_entries_both_sides_added_are_kept_in_order_and_pushed(self):
        self.branch({"src/mods.rs": RUST.replace("mod m;\n", "mod m;\nmod q;\n"),
                     "docs/index.md": DOCS.replace("- [`Z`", "- [`Sfx`](Sfx.md)\n- [`Z`")})
        self.repo.commit({"src/mods.rs": RUST.replace("mod m;\n", "mod m;\nmod p;\n"),
                          "docs/index.md": DOCS.replace("- [`Z`", "- [`Mesh`](Mesh.md)\n- [`Z`")}, "main")
        fresh, notes = self.advance()
        self.assertIsNotNone(fresh, notes)
        self.assertIn("docs/index.md, src/mods.rs", notes[0])
        self.assertEqual(self.repo.on_origin("b"), fresh)
        self.assertEqual(self.shown("src/mods.rs"), "// sorted: names only\nmod a;\nmod m;\nmod p;\nmod q;\nmod z;\n")
        self.assertIn("- [`Mesh`](Mesh.md)\n- [`Sfx`](Sfx.md)\n- [`Z`", self.shown("docs/index.md"))
        self.assertNotIn("<<<<<<<", self.shown("src/mods.rs"))
        [entry] = record.read(self.log)
        self.assertEqual((entry["ended"], entry["paths"]), (record.RESOLVED, {"docs/index.md": True, "src/mods.rs": True}))

    def test_a_clean_file_beside_a_failing_one_hands_the_whole_merge_back(self):
        self.branch({"src/mods.rs": RUST.replace("mod m;\n", "mod m;\nmod q;\n"), "src/lib.rs": "fn mine() {}\n"})
        self.repo.commit({"src/mods.rs": RUST.replace("mod m;\n", "mod m;\nmod p;\n"), "src/lib.rs": "fn theirs() {}\n"}, "main")
        fresh, notes = self.advance()
        self.assertIsNone(fresh)
        self.assertIn("Conflicted: src/lib.rs, src/mods.rs.", notes)
        self.assertEqual(notes[-1], update.HANDED_BACK)
        self.assertEqual(self.repo.on_origin("b"), self.repo.head)  # nothing pushed, nothing half-resolved
        [entry] = record.read(self.log)
        self.assertEqual((entry["ended"], entry["paths"]), (record.HANDED_BACK, {"src/lib.rs": False, "src/mods.rs": True}))

    def test_an_edited_entry_is_handed_back(self):
        self.branch({"src/mods.rs": RUST.replace("mod m;\n", "pub mod m;\nmod q;\n")})
        self.repo.commit({"src/mods.rs": RUST.replace("mod m;\n", "mod m;\nmod p;\n")}, "main")
        fresh, _ = self.advance()
        self.assertIsNone(fresh)
        self.assertEqual(record.read(self.log)[0]["ended"], record.HANDED_BACK)


if __name__ == "__main__":
    unittest.main()
