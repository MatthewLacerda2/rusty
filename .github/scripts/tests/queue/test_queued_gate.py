#!/usr/bin/env python3
"""A run whose gate is still waiting for a runner is waited on, never handed back (#1095).

PR #1063, head `66c7fa9`, 2026-10-20: every gated job of the `ci` run had
succeeded and `ci-gate` sat queued for a runner, the run read `queued`, and the
watch handed the pull request back as "no `ci` run ran a gated job". The run
finished green seconds later. Whatever the runs listing says of a run, a gate
that has not concluded is a run still going.
"""

from __future__ import annotations

import unittest

from merging.queue import outcomes
from merging.verdict import decide
from tests.queue.fixtures import SPENT, asked
from tests.verdict.fixtures import built, drafted, job, run


def gate_waiting() -> list[dict]:
    """1063's jobs at the hand-back: every gated job green, `ci-gate` not yet run."""
    return [job("ci-gate", None) if j["name"] == "ci-gate" else j for j in built("ci")]


def polled(ci: dict, ci_jobs: list[dict]) -> tuple[str, list[str]]:
    """One poll long after the push, with `lint` already green."""
    return asked(ci, run("lint"), jobs={1: ci_jobs, 2: built("lint")}, waited=SPENT)


class QueuedGate(unittest.TestCase):
    def test_1063s_run_waits_while_queued_and_passes_once_completed(self):
        going = {"status": "in_progress", "conclusion": None}
        seen = [
            polled(run("ci", **going), gate_waiting())[0],
            polled(run("ci", status="queued", conclusion=None), gate_waiting())[0],
            polled(run("ci"), built("ci"))[0],
        ]
        self.assertEqual(seen, [outcomes.WAIT, outcomes.WAIT, outcomes.GO])

    def test_a_completed_run_whose_jobs_still_show_the_gate_going_waits(self):
        # The runs listing and the jobs listing are read apart; one can be ahead.
        state, lines = polled(run("ci"), gate_waiting())
        self.assertEqual(state, outcomes.WAIT, lines)
        self.assertIn("still going", " ".join(lines))

    def test_a_completed_run_with_no_jobs_listed_yet_waits(self):
        state, lines = polled(run("ci"), [])
        self.assertEqual(state, outcomes.WAIT)
        self.assertIn("no `ci-gate` job is listed yet", " ".join(lines))

    def test_jobs_a_whole_attempt_behind_the_run_wait(self):
        # An outage cancels attempt 1 and it is re-run: the runs listing shows
        # attempt 2 green while the jobs listing still shows attempt 1.
        old = [{**j, "run_attempt": 1} for j in built("ci")]
        old = [{**j, "conclusion": "cancelled"} if j["name"] == "ci-gate" else j for j in old]
        state, lines = polled(run("ci", run_attempt=2), old)
        self.assertEqual(state, outcomes.WAIT, lines)
        self.assertIn("only for attempt 1, not the run's 2", " ".join(lines))

    def test_jobs_caught_up_with_the_attempt_pass(self):
        fresh = [{**j, "run_attempt": 2} for j in built("ci")]
        self.assertEqual(polled(run("ci", run_attempt=2), fresh)[0], outcomes.GO)

    def test_mergeable_says_wait_for_it_too(self):
        state, _ = decide.assess({"headRefOid": run("ci")["head_sha"]}, "ci", [run("ci")], {1: gate_waiting()})
        self.assertEqual(state, decide.RUNNING)

    def test_a_drafts_completed_run_is_still_unbuilt(self):
        # The gate concluded over skipped jobs: that is an answer, not a lag.
        state, lines = polled(run("ci"), drafted("ci"))
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("ran a gated job", " ".join(lines))


if __name__ == "__main__":
    unittest.main()
