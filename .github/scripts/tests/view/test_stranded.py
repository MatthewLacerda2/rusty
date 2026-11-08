#!/usr/bin/env python3
"""#1951: `make queue-status` names a ready, labelled, green pull request the
watch has passed over for more than an hour, so a stranded one (#1920, seven
hours on 2026-11-08) is an event, not something found by reading the log."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from merging.view import stranded
from merging.watch import handbacks, stalls
from tests.watch.fakes import pr

NOW = 100_000.0
GREEN, RED = (stalls.GREEN, ""), (stalls.ENDED, "red")
WHY = {"at": NOW - 7 * 3600, "why": "a `ci` run for 4f527e8 concluded failure."}


def report(pulls, reading=GREEN, why=WHY, memory=None):
    """The report over #1 remembered on `sha1` with `why`; also whether GitHub was listed."""
    asked = []
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp, handbacks.MEMORY)
        handbacks.remember(path, memory if memory is not None else {1: "sha1"})
        handbacks.remember(path.with_name(handbacks.WHY_MEMORY), {1: why} if why else {})
        said = stranded.report(path, NOW, lambda: asked.append(1) or pulls, lambda pull: reading)
    return said, bool(asked)


class Stranded(unittest.TestCase):
    def test_a_green_labelled_ready_one_passed_over_for_hours_is_named_with_its_reason(self):
        said, _ = report([pr(1)])
        self.assertEqual(len(said), 1)
        self.assertIn("STRANDED #1", said[0])
        self.assertIn("7.0 h", said[0])
        self.assertIn("concluded failure", said[0])

    def test_one_handed_back_minutes_ago_is_not_reported_and_github_is_not_asked(self):
        said, asked = report([pr(1)], why={"at": NOW - 600, "why": "x"})
        self.assertEqual((said, asked), ([], False))

    def test_a_red_head_is_not_stranded(self):
        self.assertEqual(report([pr(1)], reading=RED)[0], [])

    def test_a_draft_an_unlabelled_or_a_moved_head_is_not_stranded(self):
        for pull in (pr(1, draft=True), pr(1, queued=False), pr(1, head="moved")):
            self.assertEqual(report([pull])[0], [], pull)

    def test_one_with_no_time_recorded_counts_as_old(self):
        said, _ = report([pr(1)], why=None)
        self.assertIn("a reason not recorded", said[0])

    def test_github_failing_says_so_and_failing_a_reading_is_no_news(self):
        self.assertIn("did not list", report(None)[0][0])
        self.assertEqual(report([pr(1)], reading=None)[0], [])

    def test_no_repository_is_said_not_raised(self):
        def listing():
            raise SystemExit("no origin")
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, handbacks.MEMORY)
            handbacks.remember(path, {1: "sha1"})
            said = stranded.report(path, NOW, listing, lambda pull: GREEN)
        self.assertIn("did not list", said[0])

    def test_nothing_remembered_asks_nothing(self):
        self.assertEqual(report([pr(1)], memory={}), ([], False))


if __name__ == "__main__":
    unittest.main()
