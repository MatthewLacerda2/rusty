"""#1785: a train's tip is judged on the queue's own polling rules, and its branch's GitHub calls are REST.

[`ride.wait`] over runs faked where [`verdict.github`] reads them: green both is
go, a red `ci` is red (its halves are tested), a run that never shows by the
deadline is stalled, not red. [`rails.dispatch`] posts each gating workflow's
`dispatches`, and [`rails.left_behind`] reads only `merge-train/*` refs.
"""

from __future__ import annotations

import subprocess
import time
import unittest
from unittest import mock

from merging.queue import outcomes
from merging.train import rails, ride
from merging.verdict import github
from tests.verdict.fixtures import SHA, built, job, run


def waited(runs, jobs, deadline=3600.0):
    with mock.patch.object(github, "evidence", return_value=(runs, jobs)), \
            mock.patch.object(github, "rollup", return_value=[]), \
            mock.patch.object(time, "sleep"), \
            mock.patch.object(time, "monotonic", side_effect=[0.0, 0.0, 9999.0, 9999.0, 9999.0]):
        return ride.wait("o/r", SHA, deadline, 30, say=lambda *a: None)[0]


class Waiting(unittest.TestCase):
    def test_both_gates_green_is_go(self):
        self.assertEqual(waited([run("ci"), run("lint")], {1: built("ci"), 2: built("lint")}), outcomes.GO)

    def test_a_red_ci_is_red(self):
        red = [job("build-test", "failure"), job("ci-gate", "failure")]
        self.assertEqual(waited([run("ci", conclusion="failure"), run("lint")], {1: red, 2: built("lint")}), ride.RED)

    def test_nothing_by_the_deadline_is_stalled_not_red(self):
        self.assertEqual(waited([], {}, deadline=60.0), ride.STALLED)

    def test_a_run_cancelled_by_the_runners_is_not_the_trees_fault(self):
        cancelled = [job("build-test", "cancelled"), job("ci-gate", "failure")]
        tip = {"headRefOid": SHA, "isDraft": False}
        self.assertFalse(ride.failed(tip, [run("ci", conclusion="failure"), run("lint")], {1: cancelled, 2: built("lint")}))
        red = [job("build-test", "failure"), job("ci-gate", "failure")]
        self.assertTrue(ride.failed(tip, [run("ci", conclusion="failure"), run("lint")], {1: red, 2: built("lint")}))


class FailedPolls(unittest.TestCase):
    """One poll GitHub fails is a wait, never a stop that throws the round away (#1785's review)."""

    def poll(self, evidence, clock):
        with mock.patch.object(github, "evidence", side_effect=evidence), \
                mock.patch.object(github, "rollup", return_value=[]), \
                mock.patch.object(time, "sleep"), mock.patch.object(time, "monotonic", side_effect=clock):
            return ride.wait("o/r", SHA, 3600.0, 30, say=lambda *a: None)

    def test_a_failed_poll_waits_for_the_next(self):
        green = ([run("ci"), run("lint")], {1: built("ci"), 2: built("lint")})
        for down in (github.Unreachable("HTTP 500"), SystemExit(1)):
            with self.subTest(down=down):
                self.assertEqual(self.poll([down, green], [0.0, 0.0, 1200.0])[0], outcomes.GO)

    def test_polls_failing_to_the_deadline_are_stalled(self):
        state, lines = self.poll(github.Unreachable("HTTP 500"), [0.0, 0.0, 9999.0])
        self.assertEqual(state, ride.STALLED)
        self.assertIn("did not answer", lines[1])


class Rails(unittest.TestCase):
    def test_dispatch_posts_both_gating_workflows_over_rest(self):
        calls = []

        def runner(argv, **kw):
            calls.append(argv)
            return subprocess.CompletedProcess(argv, 0, "", "")

        self.assertIsNone(rails.dispatch("o/r", "merge-train/x", runner=runner))
        self.assertEqual([c[4] for c in calls], [f"repos/o/r/actions/workflows/{w}/dispatches" for w in ("lint.yml", "ci.yml")])
        self.assertTrue(all(c[-1] == "ref=merge-train/x" for c in calls))

    def test_a_refused_dispatch_says_why(self):
        runner = lambda argv, **kw: subprocess.CompletedProcess(argv, 1, "", "HTTP 422")
        self.assertIn("HTTP 422", rails.dispatch("o/r", "merge-train/x", runner=runner))

    def test_only_train_branches_are_left_behind(self):
        listing = "a\trefs/heads/merge-train/abc-1-2\nb\trefs/heads/main\nc\trefs/heads/1785-merge-train\n"
        self.assertEqual(rails.left_behind(listing), ["merge-train/abc-1-2"])


if __name__ == "__main__":
    unittest.main()
