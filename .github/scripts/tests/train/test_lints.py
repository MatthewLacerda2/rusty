#!/usr/bin/env python3
"""#1956: under `--no-check`, a train whose combined tree breaches a lint is split before its branch is pushed."""

from __future__ import annotations

import argparse
import os
import tempfile
import unittest
from unittest import mock

from merging.queue import lints
from merging.train import world
from tests.queue.test_markdown_advance import WHO
from tests.queue.test_update_by_merge import Origin
from tests.train.fakes import Fx, pr
from tests.train.test_ride import go


class Breach:
    """A [`lints.Linter`] stand-in: records the tree it was shown, and fails it."""

    def __init__(self, lines):
        self.lines, self.seen = lines, []

    def check(self, work, last):
        self.seen.append((os.path.isfile(os.path.join(work, "src", "lib.rs")), last))
        return [*self.lines, last]


class CombinedTree(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        env = mock.patch.dict(os.environ, WHO)
        env.start()
        self.addCleanup(env.stop)
        self.repo = Origin(self.tmp.name)

    def opts(self, **kw):
        return argparse.Namespace(**{"root": self.repo.root, "no_check": True, "dry_run": False, "target_dir": None, **kw})

    def test_no_check_lints_the_trains_tree_in_a_worktree_of_its_tip(self):
        breach = Breach(["the merged tree fails tools/lint; not pushed.", "FILE_TOO_LONG src/app/play.rs 309/300"])
        with mock.patch.object(lints, "shared", return_value=breach):
            broken = world.compiles(self.repo.head, self.opts())
        self.assertEqual(broken, [*breach.lines, lints.SPLIT])  # no blame lines, and no hand-back wording
        self.assertEqual(breach.seen, [(True, lints.SPLIT)])
        self.assertNotIn("rusty-train-", self.repo.git("worktree", "list"))

    def test_a_dry_run_lints_nothing(self):
        with mock.patch.object(lints, "shared") as shared:
            self.assertEqual(world.compiles(self.repo.head, self.opts(dry_run=True)), [])
        shared.assert_not_called()

    def test_a_breach_splits_the_train_and_pushes_nothing(self):
        line = [pr(1), pr(2)]
        fx = Fx(line)
        breach = Breach(["the merged tree fails tools/lint; not pushed.", "FILE_TOO_LONG src/a.rs 302/300"])
        fx.ns.check = lambda sha: breach.check(self.repo.root, lints.SPLIT)
        yard, _, _ = go(fx, line)
        self.assertEqual(fx.pushed, [])
        self.assertEqual(len(yard.waiting), 2)  # its halves are tested next, each alone
        self.assertTrue(any("fails tools/lint; not pushed" in said for said in fx.said))
        self.assertFalse(any("Handed back" in said or "re-admits" in said for said in fx.said))


if __name__ == "__main__":
    unittest.main()
