"""#1913: a coder whose pull request merged or closed after its fire has ended,
though its issue stays open (a `Refs #N` pull request).

On 2026-11-08 #1684's coder fired at 04:50Z and its PR #1903 (`Refs #1684`)
merged at ≈05:40Z; `make coder` counted it until 06:50Z, [`running.FIRST_PUSH`]
after the fire, and refused two re-briefs with four coders really running.
"""

from __future__ import annotations

import unittest
from datetime import timedelta

from dispatch import running
from tests import load
from tests.dispatch.test_running import FIRED, SOON, WAITS, hub, placed


def shut(number: int, closed_at: str, merged: bool) -> dict:
    return {"number": 100 + number, "head": {"ref": f"{number}-slug"}, "closed_at": closed_at,
            "merged_at": closed_at if merged else None}


#: Five minutes after FIRED's fire (03:51Z), inside FIRST_PUSH.
AFTER = "2026-11-07T03:56:00Z"
#: An earlier round's pull request, closed before the fire.
BEFORE = "2026-11-07T02:00:00Z"


class ClosedSinceTheFire(unittest.TestCase):
    def test_a_merged_refs_pull_request_has_ended(self):
        self.assertEqual(running.state(WAITS, hub(shut=[shut(2, AFTER, True)])),
                         ("ended", "its pull request merged"))

    def test_counted_without_its_closed_pull_request(self):
        # What the gate saw before #1913: the same moment, the merge not read.
        self.assertEqual(placed(WAITS, hub()), "running")

    def test_a_pull_request_closed_unmerged_after_the_fire_has_ended(self):
        self.assertEqual(running.state(FIRED, hub(shut=[shut(2, AFTER, False)])),
                         ("ended", "its pull request closed"))

    def test_a_pull_request_closed_before_the_fire_is_an_earlier_round(self):
        self.assertEqual(running.state(FIRED, hub(shut=[shut(2, BEFORE, True)])), ("running", "no pull request yet"))

    def test_the_newest_closed_pull_request_names_the_reason(self):
        later = "2026-11-07T04:00:00Z"
        both = [shut(2, AFTER, True), {**shut(2, later, False), "number": 999}]
        self.assertEqual(running.state(FIRED, hub(shut=both))[1], "its pull request closed")

    def test_another_issues_closed_pull_request_says_nothing(self):
        self.assertEqual(placed(FIRED, hub(shut=[shut(22, AFTER, True), shut(3, AFTER, True)])), "running")

    def test_an_open_draft_wins_over_a_closed_one(self):
        # A superseding `N-…-v2` draft is open: the coder on it still runs.
        self.assertEqual(placed(FIRED, hub([{"draft": True, "head": {"ref": "2-slug-v2"}}],
                                           shut=[shut(2, AFTER, False)])), "running")

    def test_past_the_first_push_a_merge_is_said_as_merged_not_stale(self):
        late = SOON + timedelta(hours=3)
        self.assertEqual(running.state(WAITS, hub(now=late, shut=[shut(2, AFTER, True)]))[1],
                         "its pull request merged")

    def test_a_closed_at_that_cannot_be_read_is_ignored(self):
        self.assertEqual(placed(FIRED, hub(shut=[{**shut(2, AFTER, True), "closed_at": "garbage"}])), "running")


class Coders(unittest.TestCase):
    def test_a_merged_refs_coder_frees_its_slot(self):
        counted, noted = running.coders([FIRED], 99, hub(shut=[shut(2, AFTER, True)]))
        self.assertEqual((counted, noted), ([], []))


class Reader(unittest.TestCase):
    def test_coder_reads_one_page_of_closed_pull_requests_once(self):
        coder = load("coder.py")
        calls: list[str] = []

        def gh(path: str):
            calls.append(path)
            if "state=closed" in path:
                return [shut(2, AFTER, True), shut(3, AFTER, False)]
            return [] if "/pulls" in path else {"state": "open"}

        reader = coder.github(gh, "o/r", SOON)
        self.assertEqual([p["number"] for p in reader.closed(2)], [102])
        self.assertEqual(reader.closed(4), [])
        closed = [c for c in calls if "state=closed" in c]
        self.assertEqual(closed, ["repos/o/r/pulls?state=closed&sort=updated&direction=desc&per_page=100"])

    def test_a_failed_listing_is_unknown_not_empty(self):
        reader = load("coder.py").github(lambda path: None if "state=closed" in path else [], "o/r", SOON)
        self.assertIsNone(reader.closed(2))


if __name__ == "__main__":
    unittest.main()
