#!/usr/bin/env python3
"""#825: Dependabot in the queue — one deadline for its rebase and its CI, and
no unattended merge of a bump that edits `.github/workflows/`.

The first keeps a slow bot pull request inside the watch's `--for` budget
(two back-to-back 45-minute waits broke it); the second keeps a green
pull-request run from standing in for workflows it never executed.
"""

from __future__ import annotations

import argparse
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

from merging.queue import cli, dependabot, gitio, outcomes, pulls, rebase, taking, waiting
from merging.watch import board, handbacks, loop, order


SHA = "a" * 40
BOTS = pulls.BOTS


def pr(number: int, *, bot=False, files=(), head=None) -> dict:
    return {
        "number": number, "isDraft": False, "headRefOid": head or f"sha{number}", "baseRefName": "main",
        "author": {"login": "app/dependabot" if bot else "someone"},
        "files": [{"path": f} for f in files], "labels": [{"name": "queue"}],
    }


class OneDeadline(unittest.TestCase):
    def test_what_is_left_is_the_deadline_less_what_was_spent(self):
        self.assertEqual(dependabot.left(45, 0), 45 * 60)
        self.assertEqual(dependabot.left(45, 30 * 60), 15 * 60)

    def test_an_overspent_deadline_leaves_nothing_never_less(self):
        self.assertEqual(dependabot.left(45, 50 * 60), 0.0)

    def take(self, author: str, rebase_minutes: float) -> float:
        """The deadline [`taking.take`] hands the CI wait, with the bot's rebase
        wait (or the local rebase) taking `rebase_minutes` of the clock."""
        clock = iter([0.0, rebase_minutes * 60])
        pull = {"number": 524, "state": "OPEN", "isDraft": False, "headRefName": "b",
                "headRefOid": SHA, "author": {"login": author}, "title": "t"}
        opts = cli.parse(["524", "--no-merge", "--deadline", "45"])
        with mock.patch.object(pulls, "look", return_value=pull), \
            mock.patch.object(gitio, "git"), \
            mock.patch.object(dependabot, "bot_head", return_value=(SHA, [])), \
            mock.patch.object(rebase, "advance", return_value=(SHA, [])), \
            mock.patch.object(time, "monotonic", side_effect=lambda: next(clock)), \
            mock.patch.object(waiting, "wait_for", return_value=(outcomes.GO, ["CI passed"])) as waited, \
            mock.patch("builtins.print"):
            taking.take("o/r", 524, opts)
        return waited.call_args.args[4]

    def test_a_bots_ci_wait_gets_only_what_its_rebase_wait_left(self):
        self.assertEqual(self.take("app/dependabot", 30), 15 * 60)

    def test_anyone_elses_ci_wait_still_gets_the_whole_deadline(self):
        self.assertEqual(self.take("someone", 30), 45 * 60)


class WorkflowBumps(unittest.TestCase):
    BUMP = (".github/workflows/ci.yml", ".github/workflows/coverage.yml")

    def test_a_bot_editing_workflows_is_held_and_names_them(self):
        self.assertEqual(board.held(pr(1, bot=True, files=self.BUMP[::-1]), BOTS), list(self.BUMP))

    def test_a_bot_editing_only_the_lockfile_is_not_held(self):
        self.assertEqual(board.held(pr(1, bot=True, files=["Cargo.lock"]), BOTS), [])

    def test_a_person_editing_workflows_is_not_held(self):
        self.assertEqual(board.held(pr(1, files=self.BUMP), BOTS), [])

    def test_the_held_bump_is_never_picked_the_lockfile_one_is(self):
        pulls = [pr(1, bot=True, files=self.BUMP), pr(2, bot=True, files=["Cargo.lock"])]
        self.assertEqual(order.pick(pulls, {}, {}, set(), BOTS)["number"], 2)
        self.assertIsNone(order.pick(pulls[:1], {}, {}, set(), BOTS))

    def test_the_watch_says_so_once_per_head_and_takes_the_rest(self):
        bump = pr(1, bot=True, files=self.BUMP)
        listings = [[bump, pr(2)], [bump], [{**bump, "headRefOid": "rebased"}]]
        said, taken, now = [], [], [0.0]

        def pulls():
            return listings.pop(0) if len(listings) > 1 else listings[0]

        def sleep(seconds):
            now[0] += seconds

        with tempfile.TemporaryDirectory() as tmp:
            fx = argparse.Namespace(
                pulls=pulls, issue_labels=dict, clashes=lambda p: set(),
                main_conflicts=lambda p: {}, turn=lambda n, tick: taken.append(n) or (n, outcomes.MERGED, "why"), head=lambda n: None,
                attempts=lambda n, sha: None,
                clock=lambda: now[0], sleep=sleep, say=lambda *a: said.append(a[0]),
                memory=Path(tmp, handbacks.MEMORY), bots=BOTS, merged=outcomes.MERGED,
                ends=set(), stops={outcomes.STOPPED}, skips={outcomes.NOT_READY},
            )
            _, why, status = loop.run(fx, cli.parse(["--watch", "--no-check", "--idle", "1"]))
        held = [line for line in said if line.startswith(board.HELD_LINE)]
        self.assertEqual(taken, [2])
        self.assertEqual(len(held), 2, held)  # one per head: `sha1`, then `rebased`
        self.assertIn(".github/workflows/ci.yml", held[0])
        self.assertEqual(status, 0)
        self.assertIn("#1", why)


if __name__ == "__main__":
    unittest.main()
