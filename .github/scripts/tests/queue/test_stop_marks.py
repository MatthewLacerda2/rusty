#!/usr/bin/env python3
"""#1951's review: every stop of the CI wait carries the verdict's runs as its mark.

#1920 sat seven hours green because its hand-back had no mark and the first
read after its green re-run became one. A refused re-run and a missing run
were two more stops left without a mark; [`waiting.stopped`] gives every stop
one from the runs the verdict read, and [`pulls.early_red`] lets the watch
re-read a red head without fetching its jobs.
"""

from __future__ import annotations

import unittest

from merging.queue import outcomes, pulls, waiting
from tests.verdict.fixtures import READY, built
from tests.verdict.fixtures import run as listed


class Stopped(unittest.TestCase):
    def test_a_red_run_is_a_red_stop_marked_with_its_runs(self):
        runs = [listed("ci", conclusion="failure"), listed("lint")]
        state, lines = waiting.stopped(READY, runs, {1: [], 2: built("lint")}, ["red"])
        self.assertEqual((state, lines.mark, list(lines)), (outcomes.RED, "1.1 2.1", ["red"]))

    def test_nothing_built_is_a_plain_stop_still_marked(self):
        state, lines = waiting.stopped(READY, [], {}, ["nothing built"])
        self.assertEqual((state, lines.mark), (outcomes.STOP, ""))

    def test_red_forced_when_the_re_run_was_refused(self):
        runs = [listed("ci"), listed("lint")]
        state, lines = waiting.stopped(READY, runs, {1: built("ci"), 2: built("lint")}, ["refused"], red=True)
        self.assertEqual((state, lines.mark), (outcomes.RED, "1.1 2.1"))

    def test_a_draft_is_never_a_red_mark(self):
        runs = [listed("ci", conclusion="failure"), listed("lint")]
        self.assertIsNone(waiting.red_mark({**READY, "isDraft": True}, runs, {1: [], 2: built("lint")}))


class EarlyRed(unittest.TestCase):
    def test_a_completed_red_run_reads_red_without_its_jobs(self):
        self.assertTrue(pulls.early_red([listed("ci", conclusion="failure"), listed("lint")]))

    def test_green_or_running_runs_do_not(self):
        running = listed("ci", status="in_progress", conclusion=None)
        self.assertFalse(pulls.early_red([listed("ci"), listed("lint")]))
        self.assertFalse(pulls.early_red([running, listed("lint")]))

    def test_a_superseded_cancelled_run_does_not(self):
        old = listed("ci", conclusion="cancelled", created_at="2026-11-08T10:00:00Z")
        new = {**listed("ci", created_at="2026-11-08T10:01:00Z"), "id": 7}
        self.assertFalse(pulls.early_red([old, new, listed("lint")]))


if __name__ == "__main__":
    unittest.main()
