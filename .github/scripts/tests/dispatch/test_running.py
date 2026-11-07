"""#1856: which coder routines count against the limit, from what the routines API lists.

The shapes are the ones the API showed for the batch's routines on 2026-11-07.
"""

from __future__ import annotations

import unittest

from dispatch import running

WAITING = {"name": "rusty #1 a", "enabled": True, "ended_reason": "", "run_once_at": "2026-11-07T04:28:00Z"}
RUNNING = {"name": "rusty #2 b", "enabled": False, "ended_reason": "run_once_fired",
           "last_fired_at": "2026-11-07T03:51:42Z",
           "last_run": {"status": "ROUTINE_RUN_STATUS_PENDING", "fired_at": "2026-11-07T03:51:42Z"}}
ENDED = {**RUNNING, "name": "rusty #3 c",
         "last_run": {"status": "ROUTINE_RUN_STATUS_SUCCEEDED", "fired_at": "2026-11-07T03:51:43Z",
                      "finished_at": "2026-11-07T04:22:04Z"}}
FAILED = {**ENDED, "name": "rusty #4 d", "last_run": {**ENDED["last_run"], "status": "ROUTINE_RUN_STATUS_FAILED"}}
NO_RUN = {"name": "rusty #5 e", "enabled": False, "ended_reason": "run_once_fired",
          "last_fired_at": "2026-11-07T03:00:00Z"}
DISABLED = {"name": "rusty #6 f", "enabled": False, "ended_reason": ""}
OTHER = {**RUNNING, "name": "scorsese #2 b"}


class State(unittest.TestCase):
    def test_each_shape_is_placed(self):
        cases = {"waiting": WAITING, "running": RUNNING, "ended": ENDED, "unknown": NO_RUN}
        for expected, found in cases.items():
            with self.subTest(expected):
                self.assertEqual(running.state(found), expected)

    def test_a_failed_run_and_a_disabled_routine_have_ended(self):
        self.assertEqual(running.state(FAILED), "ended")
        self.assertEqual(running.state(DISABLED), "ended")

    def test_a_pending_run_with_a_finish_time_has_ended(self):
        odd = {**RUNNING, "last_run": {**RUNNING["last_run"], "finished_at": "2026-11-07T04:00:00Z"}}
        self.assertEqual(running.state(odd), "ended")


class Coders(unittest.TestCase):
    def test_waiting_and_running_count_ended_does_not_and_unknown_is_set_apart(self):
        counted, unknown = running.coders([WAITING, RUNNING, ENDED, FAILED, NO_RUN, OTHER], 99)
        self.assertEqual([c["name"] for c in counted], ["rusty #1 a", "rusty #2 b"])
        self.assertEqual(unknown, [NO_RUN])

    def test_the_issue_being_launched_is_never_counted(self):
        counted, _ = running.coders([WAITING, RUNNING], 2)
        self.assertEqual(counted, [WAITING])

    def test_only_rusty_routines_count(self):
        self.assertEqual(running.issue(OTHER), None)
        self.assertEqual(running.issue({"name": "rusty #1856 x"}), 1856)
        self.assertEqual(running.coders([OTHER, {"name": "nightly"}], 1), ([], []))

    def test_the_description_says_when_it_fired_or_fires(self):
        self.assertIn("fired 2026-11-07T03:51Z", running.describe(RUNNING))
        self.assertIn("fires 2026-11-07T04:28:00Z", running.describe(WAITING))


if __name__ == "__main__":
    unittest.main()
