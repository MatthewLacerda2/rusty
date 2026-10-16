#!/usr/bin/env python3
"""Which runs on a commit count: signals beside the gates, cancelled runs, other commits."""

from __future__ import annotations

import unittest

from merging.verdict import runlist
from tests.verdict.fixtures import SHA, both, built, drafted, gates_running, job, run, signalling, verdict


class Signals(unittest.TestCase):
    """#555: `mutants-pr` shares the `ci` run but is no gate's need."""

    def in_flight(self, jobs: list[dict], **ci: object):
        ci, lint = both(status="in_progress", conclusion=None, **ci)
        return verdict(ci, lint, jobs={1: jobs, 2: built("lint")})

    def test_green_gates_beside_a_running_mutation_job_are_mergeable(self):
        ok, lines = self.in_flight(signalling(None))
        self.assertTrue(ok, lines)
        self.assertIn("Still running, and not gating: mutants-pr", " ".join(lines))

    def test_gates_still_running_are_refused_whatever_the_signals_do(self):
        ok, lines = self.in_flight([*gates_running(), job("mutants-pr", "success")])
        self.assertFalse(ok, lines)
        self.assertIn("in_progress", " ".join(lines))

    def test_a_red_signal_never_blocks(self):
        # Mid-run, and after: `continue-on-error` leaves the finished run green.
        self.assertTrue(self.in_flight(signalling("failure"))[0])
        ok, lines = verdict(*both(), jobs={1: signalling("failure"), 2: built("lint")})
        self.assertTrue(ok, lines)

    def test_a_finished_red_run_is_not_forgiven_by_a_green_gate(self):
        ok, _ = verdict(*both(conclusion="failure"), jobs={1: signalling("success"), 2: built("lint")})
        self.assertFalse(ok)

    def test_a_red_gate_beside_a_running_signal_is_a_failure_now(self):
        jobs = [*built("ci")[:-2], job("ci-gate", "failure"), job("mutants-pr", None)]
        ok, lines = self.in_flight(jobs)
        self.assertFalse(ok, lines)
        self.assertIn("concluded failure", " ".join(lines))

    def test_a_drafts_run_is_still_unbuilt_while_a_signal_runs(self):
        ok, lines = self.in_flight([*drafted("ci"), job("mutants-pr", None)])
        self.assertFalse(ok, lines)
        self.assertIn("ran a gated job", " ".join(lines))

    def test_the_api_doc_still_needs_its_drift_run_while_a_signal_runs(self):
        skipped = [
            job("build-test", "success", [{"name": "Test (engine, dev features)", "conclusion": "skipped"}]),
            job("ci-gate", "success"),
            job("mutants-pr", None),
        ]
        ok, lines = self.in_flight(skipped)
        self.assertTrue(ok, lines)  # code files: the drift rule does not apply
        ci, lint = both(status="in_progress", conclusion=None)
        ok, lines = verdict(ci, lint, jobs={1: skipped, 2: built("lint")}, files=["docs/api/Physics.md"])
        self.assertFalse(ok, lines)
        self.assertIn("#525", " ".join(lines))

    def test_a_cancelled_run_with_a_skipped_gate_still_reads_cancelled(self):
        ci, lint = both(conclusion="cancelled")
        jobs = {1: [job("build-test", "cancelled"), job("ci-gate", "skipped")], 2: built("lint")}
        ok, lines = verdict(ci, lint, jobs=jobs)
        self.assertFalse(ok)
        self.assertIn("concluded cancelled", " ".join(lines))


class Cancelled(unittest.TestCase):
    """#482 cancels a superseded PR run; until #515 its gate reads red."""

    def test_a_cancelled_run_superseded_on_the_same_commit_is_dropped(self):
        old = run("ci", id=1, conclusion="cancelled", created_at="2026-09-30T00:00:00Z")
        new = run("ci", id=3)
        jobs = {1: [job("ci-gate", "failure")], 3: built("ci"), 2: built("lint")}
        ok, lines = verdict(old, new, run("lint"), jobs=jobs)
        self.assertTrue(ok, lines)
        self.assertIn("1 cancelled and superseded", " ".join(lines))

    def test_the_order_listed_does_not_decide_which_is_newer(self):
        old = run("ci", id=1, conclusion="cancelled", created_at="2026-09-30T00:00:00Z")
        new = run("ci", id=3)
        jobs = {1: [], 3: built("ci"), 2: built("lint")}
        self.assertTrue(verdict(new, old, run("lint"), jobs=jobs)[0])
        self.assertTrue(verdict(old, new, run("lint"), jobs=jobs)[0])

    def test_a_cancelled_run_that_is_the_newest_still_refuses(self):
        # Nothing replaced its answer, so nothing has answered.
        ok, lines = verdict(*both(conclusion="cancelled"))
        self.assertFalse(ok)
        self.assertIn("cancelled", " ".join(lines))

    def test_superseded_by_a_live_run_means_wait_for_that_run(self):
        old = run("ci", id=1, conclusion="cancelled", created_at="2026-09-30T00:00:00Z")
        new = run("ci", id=3, status="in_progress", conclusion=None)
        ok, lines = verdict(old, new, run("lint"), jobs={1: [], 3: [], 2: built("lint")})
        self.assertFalse(ok)
        self.assertIn("in_progress", " ".join(lines))

    def test_558s_run_set_waits_for_the_live_run_not_red(self):
        # #562, verbatim from #558's head: same workflow, same second, and the
        # cancelled run carries the *higher* id — id order cannot decide it.
        at = "2026-09-30T15:44:14Z"
        live = run("lint", id=36738997580, created_at=at, status="in_progress", conclusion=None)
        dead = run("lint", id=36738997790, created_at=at, conclusion="cancelled")
        ci_dead = run("ci", id=36738997830, created_at=at, conclusion="cancelled")
        ci_live = run("ci", id=36738998031, created_at="2026-09-30T15:44:15Z", status="pending", conclusion=None)
        jobs = {r["id"]: [] for r in (live, dead, ci_dead, ci_live)}
        ok, lines = verdict(live, dead, ci_dead, ci_live, jobs=jobs)
        self.assertFalse(ok)
        self.assertIn("wait for it", " ".join(lines).lower())
        self.assertNotIn("beside a red one", " ".join(lines))

    def test_558s_run_set_passes_once_the_live_run_is_green(self):
        at = "2026-09-30T15:44:14Z"
        done = run("lint", id=36738997580, created_at=at)
        dead = run("lint", id=36738997790, created_at=at, conclusion="cancelled")
        jobs = {1: built("ci"), done["id"]: built("lint"), dead["id"]: []}
        ok, lines = verdict(run("ci"), done, dead, jobs=jobs)
        self.assertTrue(ok, lines)

    def test_a_cancelled_run_newer_than_a_green_one_still_refuses(self):
        green = run("ci", id=1, created_at="2026-09-30T00:00:00Z")
        dead = run("ci", id=3, conclusion="cancelled")
        ok, lines = verdict(green, dead, run("lint"), jobs={1: built("ci"), 3: [], 2: built("lint")})
        self.assertFalse(ok)
        self.assertIn("cancelled", " ".join(lines))

    def test_cancelled_runs_never_supersede_each_other(self):
        at = "2026-09-30T00:00:00Z"
        a, b = (run("ci", id=i, created_at=at, conclusion="cancelled") for i in (1, 3))
        self.assertEqual(runlist.superseded([a, b]), [])

    def test_only_cancelled_runs_are_ever_superseded(self):
        red = run("ci", id=1, conclusion="failure", created_at="2026-09-30T00:00:00Z")
        self.assertEqual(runlist.superseded([red, run("ci", id=3)]), [])


class Picking(unittest.TestCase):
    def test_a_run_for_another_commit_is_not_this_commits(self):
        self.assertEqual(runlist.runs_for([run(head_sha="0" * 40)], SHA, "ci"), [])

    def test_other_workflows_are_not_asked_about(self):
        # `main-health` is a `workflow_run` on `main`, never a PR check.
        for name in ("main-health", "docs"):
            self.assertEqual(runlist.runs_for([run(name)], SHA, "ci"), [])

    def test_a_matrix_job_counts_under_its_base_name(self):
        self.assertEqual(runlist.base("build-test-cross (macos-latest)"), "build-test-cross")

    def test_a_skipped_run_is_not_a_failed_one(self):
        self.assertEqual(runlist.failed_runs([run(conclusion="skipped")]), [])


if __name__ == "__main__":
    unittest.main()
