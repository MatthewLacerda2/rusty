#!/usr/bin/env python3
"""#592: a run the queue has seen does not stop existing when the listing blinks.

`actions/runs?head_sha=` is eventually consistent. On #589 it showed a live
`ci` run six times, nothing once, the run three more times, then nothing past
[`polling.RUN_APPEARS_SECONDS`] — and the queue handed the PR back advising an
empty commit, which cancelled the real run. These drive [`waiting.wait_for`]
over that sequence with `gh` and the clock replaced.
"""

from __future__ import annotations

import unittest
from unittest import mock

from merging.queue import outcomes, polling, pulls, waiting
from merging.verdict import github

from tests.verdict import fixtures
from tests.verdict.fixtures import SHA, built, run

PULL = {**fixtures.READY, "files": [{"path": "src/app/mod.rs"}]}
POLL, DEADLINE = 30.0, 45 * 60.0
LINT = run("lint")
RUNNING = ([run("ci", status="in_progress", conclusion=None), LINT], {1: fixtures.gates_running(), 2: built("lint")})
PASSED = ([run("ci"), LINT], {1: built("ci"), 2: built("lint")})
EMPTY = ([], {})
# #589's log, one entry per poll: the empty spells come after the grace is spent.
SEEN_ON_589 = [RUNNING] * 6 + [EMPTY] + [RUNNING] * 3 + [EMPTY] * 5


class Clock:
    """`time` for the queue: `sleep` moves `monotonic` on, nothing waits."""

    def __init__(self):
        self.now = 0.0

    def monotonic(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.now += seconds


def waited(listings: list, pull: dict = PULL):
    """[`waiting.wait_for`] over these listings; the last one repeats forever."""
    polls = iter(listings)
    last = [listings[-1]]

    def evidence(_repo, _sha):
        last[0] = next(polls, last[0])
        return last[0]

    with mock.patch.object(pulls, "look", return_value=pull), \
        mock.patch.object(github, "evidence", side_effect=evidence), \
        mock.patch.object(waiting, "time", Clock()), \
        mock.patch("builtins.print"):
        return waiting.wait_for("o/r", 589, SHA, SHA, DEADLINE, POLL)


class Flicker(unittest.TestCase):
    def test_589s_sequence_then_a_pass_merges(self):
        self.assertGreater(len(SEEN_ON_589) * POLL, polling.RUN_APPEARS_SECONDS)
        state, lines = waited([*SEEN_ON_589, PASSED])
        self.assertEqual(state, outcomes.GO, lines)

    def test_a_run_that_never_comes_back_is_handed_back_naming_the_flicker(self):
        state, lines = waited([RUNNING, EMPTY])
        said = " ".join(lines)
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("dropped out of the runs listing", said)
        self.assertIn("minutes", said)
        self.assertNotIn("empty commit", said)
        self.assertNotIn("no `ci` run exists", said)

    def test_a_run_never_seen_is_still_absent_once_the_grace_is_spent(self):
        state, lines = waited([EMPTY])
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("no `ci` run exists", " ".join(lines))

    def test_the_prs_own_checks_count_as_having_seen_the_run(self):
        checks = [{"__typename": "CheckRun", "name": w, "workflowName": w} for w in ("ci", "lint")]
        state, lines = waited([EMPTY] * 20 + [PASSED], pull={**PULL, "statusCheckRollup": checks})
        self.assertEqual(state, outcomes.GO, lines)


class OneShot(unittest.TestCase):
    """`make mergeable` never saw a run, but the PR's checks can vouch for one."""

    def test_a_lagging_listing_says_ask_again_not_empty_commit(self):
        pull = {**fixtures.READY, "statusCheckRollup": [{"name": "build-test", "workflowName": "ci"}]}
        ok, lines = fixtures.verdict(LINT, pull=pull)
        said = " ".join(lines)
        self.assertFalse(ok)
        self.assertIn("Ask again", said)
        self.assertNotIn("Force one", said)

    def test_no_checks_anywhere_is_still_scorsese_153(self):
        ok, lines = fixtures.verdict(LINT)
        self.assertFalse(ok)
        self.assertIn("Force one with an empty commit", " ".join(lines))


if __name__ == "__main__":
    unittest.main()
