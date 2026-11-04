#!/usr/bin/env python3
"""#1831: what the runs on a stalled head read as ([`stalls.reading`]).

A take that outwaited its runs ([`outcomes.STALLED`]) comes back only when no
gating run on its head is still out and every one concluded success; a run
listed `completed` whose jobs don't yet agree is still out (#1095, #1182). A
cancelled or red ending is said, never re-run.
"""

from __future__ import annotations

import unittest

from merging.watch import stalls
from tests.verdict.fixtures import READY, built, gates_running, job, run


def read(ci: dict, ci_jobs: list[dict], lint: dict | None = None, lint_jobs: list[dict] | None = None):
    jobs = {1: ci_jobs, 2: lint_jobs if lint_jobs is not None else built("lint")}
    return stalls.reading(READY, [ci, lint or run("lint")], jobs)


class Reading(unittest.TestCase):
    def test_every_run_green_is_green(self):
        self.assertEqual(read(run("ci"), built("ci")), (stalls.GREEN, ""))

    def test_a_run_still_out_is_out(self):
        self.assertEqual(read(run("ci", status="in_progress", conclusion=None), gates_running())[0], stalls.OUT)

    def test_one_run_green_beside_one_still_out_is_out(self):
        lint = run("lint", status="queued", conclusion=None)
        self.assertEqual(read(run("ci"), built("ci"), lint, [])[0], stalls.OUT)

    def test_a_cancelled_ending_is_ended_with_its_reason(self):
        cancelled = [job("changes", "cancelled"), job("build-test", "cancelled"), job("ci-gate", "failure")]
        state, why = read(run("ci", conclusion="cancelled"), cancelled)
        self.assertEqual(state, stalls.ENDED)
        self.assertIn("concluded cancelled", why)

    def test_a_red_ending_is_ended_with_its_reason(self):
        red = [job("changes", "success"), job("build-test", "failure"), job("ci-gate", "failure")]
        state, why = read(run("ci", conclusion="failure"), red)
        self.assertEqual(state, stalls.ENDED)
        self.assertIn("concluded failure", why)

    def test_a_red_beside_a_run_still_out_is_ended_already(self):
        lint = run("lint", status="in_progress", conclusion=None)
        red = [job("build-test", "failure"), job("ci-gate", "failure")]
        self.assertEqual(read(run("ci", conclusion="failure"), red, lint, [])[0], stalls.ENDED)

    def test_the_lag_shape_is_out_until_the_jobs_agree(self):
        lagging = [*built("ci")[:-2], {"name": "ci-gate", "status": "in_progress", "conclusion": None}]
        self.assertEqual(read(run("ci"), lagging)[0], stalls.OUT)
        self.assertEqual(read(run("ci"), built("ci"))[0], stalls.GREEN)

    def test_a_run_gone_from_the_listing_is_ended(self):
        state, why = stalls.reading(READY, [run("lint")], {2: built("lint")})
        self.assertEqual(state, stalls.ENDED)
        self.assertIn("`ci`", why)


class Returned(unittest.TestCase):
    def ask(self, answer, pull=READY, memory=None):
        asked = []
        got = stalls.returned([pull], memory or {524: READY["headRefOid"]}, {524: "s"}, lambda p: asked.append(p) or answer)
        return got, asked

    def test_github_not_answering_is_no_news(self):
        self.assertEqual(self.ask(None)[0], ([], []))

    def test_green_returns_and_an_ending_is_reported(self):
        self.assertEqual(self.ask((stalls.GREEN, ""))[0], ([524], []))
        self.assertEqual(self.ask((stalls.ENDED, "why"))[0], ([], [(524, "why")]))
        self.assertEqual(self.ask((stalls.OUT, ""))[0], ([], []))

    def test_a_draft_or_a_moved_head_is_not_asked(self):
        self.assertEqual(self.ask((stalls.GREEN, ""), pull={**READY, "isDraft": True}), (([], []), []))
        self.assertEqual(self.ask((stalls.GREEN, ""), memory={524: "other"}), (([], []), []))

    def test_a_hand_back_that_is_no_stall_is_not_asked(self):
        asked = []
        got = stalls.returned([READY], {524: READY["headRefOid"]}, {}, lambda p: asked.append(p) or (stalls.GREEN, ""))
        self.assertEqual((got, asked), (([], []), []))


if __name__ == "__main__":
    unittest.main()
