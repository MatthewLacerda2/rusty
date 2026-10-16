#!/usr/bin/env python3
"""The queue's bookkeeping: order, the summary, the squash title, and its command line."""

from __future__ import annotations

import subprocess
import sys
import unittest

from merging.queue import outcomes, pulls
from tests import SCRIPTS
from tests.queue.fixtures import READY

SCRIPT = SCRIPTS / "merge-queue.py"


class Queueing(unittest.TestCase):
    def test_order_is_kept_and_repeats_dropped(self):
        self.assertEqual(outcomes.ordered([486, 488, 486, 489]), [486, 488, 489])

    def test_merged_green_and_dry_are_reported_differently(self):
        lines = outcomes.summary([(1, outcomes.MERGED, "a"), (2, outcomes.GREEN, "b"), (3, outcomes.DRY, "c")])
        self.assertIn("#1: merged", lines[0])
        self.assertIn("#2: green", lines[1])
        self.assertIn("#3: dry run", lines[2])

    def test_merged_branches_are_named_for_cleanup_rather_than_cleaned(self):
        lines = outcomes.summary([(486, outcomes.MERGED, "CI passed")])
        self.assertIn("#486", lines[-1])
        self.assertIn(".claude/worktrees/", lines[-1])

    def test_a_hand_back_has_no_cleanup_note(self):
        self.assertEqual(len(outcomes.summary([(486, outcomes.HANDED_BACK, "draft")])), 1)

    def test_the_squash_title_carries_the_pr_number_once(self):
        # House style: `Title (#issue) (#pr)`, and never `(#pr) (#pr)`.
        self.assertEqual(pulls.subject({"title": "Fix it (#12)", "number": 30}), "Fix it (#12) (#30)")
        self.assertEqual(pulls.subject({"title": "Fix it (#30)", "number": 30}), "Fix it (#30)")

    def test_dependabot_is_recognised_by_either_spelling(self):
        for login in ("app/dependabot", "dependabot[bot]"):
            self.assertTrue(pulls.is_bot({"author": {"login": login}}))
        self.assertFalse(pulls.is_bot(READY))


class Contract(unittest.TestCase):
    def test_it_asks_for_at_least_one_pull_request(self):
        done = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True, check=False)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("usage", done.stderr.lower())

    def test_every_flag_carries_help_text(self):
        done = subprocess.run([sys.executable, str(SCRIPT), "--help"], capture_output=True, text=True, check=False)
        self.assertEqual(done.returncode, 0)
        for flag, described in (
            ("--no-merge", "hand each branch back"),
            ("--dry-run", "merge nothing"),
            ("--deadline", "give up waiting"),
            ("--poll", "ask GitHub"),
            ("--root", "rebase in"),
        ):
            self.assertIn(flag, done.stdout)
            self.assertIn(described, " ".join(done.stdout.split()))


if __name__ == "__main__":
    unittest.main()
