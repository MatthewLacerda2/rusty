#!/usr/bin/env python3
"""#969: two ordering rules the watch applies from data it already lists.

- **One `Cargo.toml`/`Cargo.lock` owner at a time**: when two open pull
  requests (drafts included) edit either, the younger waits, with one
  `LOCKFILE` line, until the older merges or closes. Dependabot is exempt.
- **A bug in the dev tooling ranks as infrastructure**: a `bug` pull request
  whose files all sit under the tooling paths.
"""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.watch import board, loop, order
from tests.watch import fakes
from tests.watch.test_select import BOTS, pr


def editing(number, *paths, **kw):
    pull = pr(number, **kw)
    pull["files"] = [{"path": p} for p in paths]
    return pull


class Lockfile(unittest.TestCase):
    def test_the_younger_waits_on_the_older_draft_included(self):
        pulls = [editing(12, "Cargo.lock"), editing(10, "Cargo.toml", draft=True), editing(11, "src/a.rs")]
        self.assertEqual(board.lock_waits(pulls, BOTS), {12: 10})
        self.assertEqual([p["number"] for p in order.ready(pulls, {}, set(), BOTS)], [11])

    def test_one_editor_or_dependabot_holds_nobody(self):
        self.assertEqual(board.lock_waits([editing(10, "Cargo.lock"), editing(11, "README.md")], BOTS), {})
        bumps = [editing(9, "Cargo.lock", bot=True), editing(10, "Cargo.lock")]
        self.assertEqual(board.lock_waits(bumps, BOTS), {})
        self.assertEqual(len(order.ready(bumps, {}, set(), BOTS)), 2)

    def test_in_the_loop_the_waiter_goes_once_the_owner_merges(self):
        with tempfile.TemporaryDirectory() as tmp:
            pulls = [editing(10, "Cargo.lock"), editing(11, "Cargo.lock")]
            world = fakes.World(tmp, [pulls], {10: outcomes.MERGED, 11: outcomes.MERGED})
            said = []
            world.fx.say = said.append
            loop.run(world.fx, fakes.opts())
        self.assertEqual(world.taken, [10, 11])
        self.assertEqual([s for s in said if board.LOCKFILE_LINE in s], [board.lock_line(11, 10)])

    def test_a_draft_owner_keeps_a_ready_waiter_out(self):
        with tempfile.TemporaryDirectory() as tmp:
            pulls = [editing(10, "Cargo.toml", draft=True), editing(11, "Cargo.lock")]
            world = fakes.World(tmp, [pulls], {11: outcomes.MERGED})
            loop.run(world.fx, fakes.opts(idle=1, poll=30))
        self.assertEqual(world.taken, [])


class ToolingBug(unittest.TestCase):
    ISSUES = {1: {"bug"}, 2: {"architecture"}}

    def first(self, *pulls):
        return order.pick(list(pulls), self.ISSUES, {}, set(), BOTS)["number"]

    def test_a_bug_only_in_tooling_ranks_as_infrastructure(self):
        for path in (".github/workflows/ci.yml", "Makefile", ".gitignore", "tools/lint/src/x.rs", ".claude/skills/a.md"):
            with self.subTest(path=path):
                self.assertEqual(self.first(editing(20, "src/a.rs", closes=[2]), editing(21, path, closes=[1])), 21)

    def test_a_bug_touching_the_engine_keeps_its_rank(self):
        self.assertEqual(self.first(editing(20, "src/a.rs", closes=[2]), editing(21, "Makefile", "src/b.rs", closes=[1])), 20)

    def test_tooling_without_the_bug_label_is_not_promoted(self):
        self.assertEqual(self.first(editing(20, "src/a.rs", closes=[2]), editing(21, "Makefile")), 20)


if __name__ == "__main__":
    unittest.main()
