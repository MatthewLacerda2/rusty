"""#1785: the train's tree is the tree squash-merging its members in order leaves.

Over real repositories ([`tests.queue.test_update_by_merge.Origin`]): the chain
[`build.couple`] writes is compared with `git merge --squash`, member by member,
which is what GitHub's squash merge does; a member that conflicts with the
chain stays off it; and the `merge-train/*` branches are pushed, found and
swept from a real bare `origin` ([`rails`]).
"""

from __future__ import annotations

import os
import tempfile
import unittest
from unittest import mock

from merging.queue import world
from merging.train import build, group, rails
from tests.queue.test_markdown_advance import WHO
from tests.queue.test_update_by_merge import Origin


class Building(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        env = mock.patch.dict(os.environ, WHO)
        env.start()
        self.addCleanup(env.stop)
        self.repo = Origin(tmp.name)  # `main` with src/lib.rs, `b` adding src/feature.rs
        self.repo.git("checkout", "--quiet", "-b", "c", "main")
        self.c = self.repo.commit({"src/other.rs": "fn o() {}\n"}, "c")
        self.repo.git("checkout", "--quiet", "-b", "d", "main")
        self.d = self.repo.commit({"src/feature.rs": "fn clash() {}\n"}, "d")  # conflicts with `b`
        self.repo.git("checkout", "--quiet", "main")
        self.git = lambda *args: world.merge_tree_git(self.repo.root, *args)

    def squashed(self, heads: list[str]) -> str:
        """`main`'s tree after `git merge --squash` of each head in turn."""
        self.repo.git("checkout", "--quiet", "-B", "landing", "origin/main")
        for head in heads:
            self.repo.git("merge", "--squash", "--quiet", head)
            self.repo.git("commit", "--quiet", "-m", f"squash {head[:7]}")
        return self.repo.git("rev-parse", "HEAD^{tree}").strip()

    def test_the_chain_is_the_tree_squashing_each_member_leaves(self):
        line = [{"number": 1, "headRefOid": self.repo.head}, {"number": 2, "headRefOid": self.c}]
        members, formed, notes = group.form(line, build.start(self.git), self.git, lambda pull: None)
        self.assertEqual(([p["number"] for p in members], notes), ([1, 2], []))
        self.assertEqual(formed.tree, self.squashed([self.repo.head, self.c]))
        self.assertEqual(formed.tree_after(1), self.squashed([self.repo.head]))
        # Single-parent commits only, so no branch commit rides along to change a merge base.
        parents = self.repo.git("rev-list", "--parents", "-n", "1", formed.tip).split()
        self.assertEqual(parents[1:], [formed.cars[0].commit])

    def test_a_member_conflicting_with_the_chain_stays_off_it_and_waits(self):
        line = [{"number": n, "headRefOid": h} for n, h in ((1, self.repo.head), (2, self.d), (3, self.c))]
        members, formed, notes = group.form(line, build.start(self.git), self.git, lambda pull: None)
        self.assertEqual(formed.numbers, [1, 3])
        self.assertIn("#2 waits: it conflicts with the train so far (#1)", notes[0])
        self.assertEqual(formed.tree, self.squashed([self.repo.head, self.c]))

    def test_branches_are_pushed_found_and_swept(self):
        train = build.start(self.git)
        train.cars.append(build.couple(train, 7, self.c, self.git))
        self.assertIsNone(rails.push(train, self.repo.root))
        self.assertIsNone(rails.push(train, self.repo.root))  # one left behind is replaced, never forced
        self.assertEqual(self.repo.on_origin(rails.branch(train)), train.tip)
        self.assertEqual(rails.sweep(self.repo.root), [rails.branch(train)])
        self.assertEqual(rails.sweep(self.repo.root), [])


if __name__ == "__main__":
    unittest.main()
