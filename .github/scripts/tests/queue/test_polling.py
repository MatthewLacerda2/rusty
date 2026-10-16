#!/usr/bin/env python3
"""The queue waits, and never mistakes *not yet* for either answer.

It is asked repeatedly of a commit it pushed seconds ago, so an absence of
evidence starts out meaning *GitHub has not caught up* and ends up meaning
scorsese#153. Getting that backwards either refuses every branch it pushes or
merges one nothing compiled. Only a red run is trusted at once. Nothing here
talks to `gh` or `git`: the end-to-end cases patch both out.
"""

from __future__ import annotations

import unittest

from merging.queue import outcomes, polling, rebase
from tests.queue.fixtures import READY, SPENT, asked
from tests.verdict import fixtures
from tests.verdict.fixtures import SHA, built, drafted, run


class Polling(unittest.TestCase):
    def test_a_missing_run_is_not_yet_an_answer_right_after_a_push(self):
        state, lines = asked(waited=1.0)
        self.assertEqual(state, outcomes.WAIT)
        self.assertIn("nothing has built", " ".join(lines))

    def test_one_workflow_present_and_the_other_not_yet_is_waited_on(self):
        self.assertEqual(asked(run("ci"), waited=1.0)[0], outcomes.WAIT)

    def test_a_listing_that_shows_only_the_drafts_run_is_absence(self):
        # scorsese watched the runs listing omit a live run beside a skipped one.
        state, _ = asked(run("ci"), run("lint"), jobs={1: drafted("ci"), 2: built("lint")}, waited=1.0)
        self.assertEqual(state, outcomes.WAIT)

    def test_that_grace_does_not_outlast_the_window(self):
        state, lines = asked(run("ci"), run("lint"), jobs={1: drafted("ci"), 2: built("lint")}, waited=SPENT)
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("ran a gated job", " ".join(lines))

    def test_a_missing_run_becomes_the_answer_once_the_grace_is_spent(self):
        state, lines = asked(waited=SPENT)
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("no `ci` run exists", " ".join(lines))

    def test_a_red_run_stops_inside_the_grace(self):
        state, lines = asked(run("ci", conclusion="failure"), waited=0.0)
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("failure", " ".join(lines))

    def test_a_failure_stops_even_with_a_live_run_beside_it(self):
        state, _ = asked(run("ci", conclusion="failure"), run("lint", status="in_progress", conclusion=None))
        self.assertEqual(state, outcomes.STOP)

    def test_a_run_in_flight_is_waited_on(self):
        queued = run("ci", status="queued", conclusion=None)
        state, lines = asked(queued, run("lint"), jobs={1: fixtures.gates_running(), 2: built("lint")})
        self.assertEqual(state, outcomes.WAIT)
        self.assertIn("queued", " ".join(lines))

    def test_green_gates_are_not_held_by_a_running_mutation_job(self):
        # #555: the signal shares the `ci` run; the queue must not wait it out.
        busy = run("ci", status="in_progress", conclusion=None)
        state, lines = asked(busy, run("lint"), jobs={1: fixtures.signalling(None), 2: built("lint")})
        self.assertEqual(state, outcomes.GO, lines)

    def test_a_superseded_cancelled_run_does_not_stop_the_queue(self):
        old = run("ci", id=1, conclusion="cancelled", created_at="2026-09-30T00:00:00Z")
        state, _ = asked(old, run("ci", id=3), run("lint"), jobs={1: [], 3: built("ci"), 2: built("lint")})
        self.assertEqual(state, outcomes.GO)

    def test_a_draft_is_stopped_and_not_waited_on(self):
        self.assertEqual(asked(run("ci"), run("lint"), pull={**READY, "isDraft": True})[0], outcomes.STOP)

    def test_a_genuine_pass_goes(self):
        state, lines = asked(run("ci"), run("lint"))
        self.assertEqual(state, outcomes.GO)
        self.assertIn("passed", lines[0])

    def test_the_api_doc_refusal_is_mergeables_and_stops(self):
        # Whatever `judge` refuses, this refuses: one definition of a pass.
        jobs = {1: [fixtures.job("build-test", "success"), fixtures.job("ci-gate", "success")], 2: built("lint")}
        state, lines = asked(run("ci"), run("lint"), jobs=jobs, files=["docs/api/Physics.md"])
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("#525", " ".join(lines))


class Rebasing(unittest.TestCase):
    def test_a_rebase_that_moves_nothing_is_not_pushed(self):
        self.assertFalse(rebase.push_needed(SHA, SHA))
        self.assertTrue(rebase.push_needed(SHA, "0" * 40))

    def test_the_pre_push_head_is_github_lagging(self):
        state, lines = polling.head_state(SHA, "1" * 40, SHA, waited=1.0)
        self.assertEqual(state, outcomes.WAIT)
        self.assertIn("not caught up", " ".join(lines))

    def test_the_pre_push_head_stops_being_an_excuse_after_the_grace(self):
        self.assertEqual(polling.head_state(SHA, "1" * 40, SHA, waited=SPENT)[0], outcomes.STOP)

    def test_a_third_sha_is_somebody_else_and_stops_immediately(self):
        state, lines = polling.head_state("2" * 40, "1" * 40, SHA, waited=0.0)
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("Somebody else pushed", " ".join(lines))

    def test_the_watched_head_goes(self):
        self.assertEqual(polling.head_state(SHA, SHA, SHA, 0.0)[0], outcomes.GO)

    def test_conflicted_paths_are_named(self):
        named = rebase.conflicts("src/components/kind.rs\nsrc/app/registry.rs\n\n")
        self.assertEqual(named, ["src/components/kind.rs", "src/app/registry.rs"])


if __name__ == "__main__":
    unittest.main()
