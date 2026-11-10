"""#1856, #1866: which coder routines count against the limit.

The routine shapes are the ones the API showed for the batch's routines on
2026-11-07; a fired coder is placed from its pull request and its briefs.
"""

from __future__ import annotations

import unittest
from datetime import datetime, timedelta, timezone

from dispatch import running

FIRE = "2026-11-07T03:51:42.123Z"
WAITING = {"name": "rusty #1 a", "enabled": True, "ended_reason": "", "run_once_at": "2026-11-07T04:28:00Z"}
FIRED = {"name": "rusty #2 b", "enabled": False, "ended_reason": "run_once_fired", "last_fired_at": FIRE,
         "last_run": {"status": "ROUTINE_RUN_STATUS_PENDING", "fired_at": FIRE}}
#: #1844's coder on 2026-11-07: waiting on a mutation run, its run reads finished.
WAITS = {**FIRED, "last_run": {"status": "ROUTINE_RUN_STATUS_SUCCEEDED", "fired_at": FIRE,
                               "finished_at": "2026-11-07T04:22:04Z"}}
NO_TIME = {"name": "rusty #5 e", "enabled": False, "ended_reason": "run_once_fired"}
DISABLED = {"name": "rusty #6 f", "enabled": False, "ended_reason": ""}
#: Ten minutes after FIRE: a coder still inside its first push.
SOON = datetime(2026, 11, 7, 4, 1, 42, tzinfo=timezone.utc)


def note(body: str, at: str, who: str = "OWNER") -> dict:
    return {"body": body, "created_at": at, "author_association": who}


#: A review record on `pr(2, …)` and `pr(3, …)`'s conversations (#2017).
REVIEWED = {n: [note("Independent review (high)", "2026-11-07T05:00:00Z")] for n in (102, 103)}


def pr(number: int, draft: bool) -> dict:
    return {"number": 100 + number, "draft": draft, "head": {"ref": f"{number}-slug"}}


def hub(pulls=(), comments=None, closed=(), now=SOON, shut=()) -> running.GitHub:
    """`closed` names closed issues; `shut` lists closed pull requests (any issue's)."""
    return running.GitHub(pulls=list(pulls), comments=lambda n: (comments or {}).get(n, []),
                          issue=lambda n: {"state": "closed" if n in closed else "open"}, now=now,
                          closed=lambda n: running.pulls_for(n, list(shut)))


def placed(found: dict, github: running.GitHub) -> str:
    return running.state(found, github)[0]


class State(unittest.TestCase):
    def test_not_fired_is_running_and_disabled_has_ended(self):
        self.assertEqual(placed(WAITING, hub()), "running")
        self.assertEqual(placed(DISABLED, hub()), "ended")

    def test_a_draft_counts_whatever_the_run_says(self):
        for found in (FIRED, WAITS):
            with self.subTest(found["last_run"]["status"]):
                self.assertEqual(placed(found, hub([pr(2, True)])), "running")

    def test_a_ready_pull_request_has_ended_once_reviewed(self):
        self.assertEqual(placed(FIRED, hub([pr(2, False)], REVIEWED)), "ended")

    def test_a_merged_coder_has_ended_though_its_session_is_active(self):
        self.assertEqual(placed(WAITS, hub([], closed={2})), "ended")

    def test_a_coder_with_no_pull_request_yet_counts(self):
        self.assertEqual(running.state(FIRED, hub()), ("running", "no pull request yet"))

    def test_no_pull_request_past_the_first_push_has_ended(self):
        # #1714 on 2026-11-07: part 2 merged (`Refs`), the issue open, part 3 not launched.
        late = SOON + timedelta(hours=3)
        self.assertEqual(running.state(WAITS, hub(now=late)), ("ended", running.STALE))
        edge = running.when(FIRE) + running.FIRST_PUSH
        self.assertEqual(placed(FIRED, hub(now=edge - timedelta(seconds=1))), "running")
        self.assertEqual(placed(FIRED, hub(now=edge)), "ended")

    def test_a_draft_counts_past_the_first_push(self):
        self.assertEqual(placed(FIRED, hub([pr(2, True)], now=SOON + timedelta(days=1))), "running")

    def test_a_newer_brief_is_a_hand_back_waiting_for_relaunch(self):
        later = {2: [note("Brief\n\nBranch: x", "2026-11-07T05:00:00Z")]}
        self.assertEqual(placed(FIRED, hub([pr(2, True)], later)), "ended")

    def test_an_older_brief_or_an_untrusted_one_does_not_end_it(self):
        cases = {"older": note("Brief", "2026-11-07T03:00:00Z"),
                 "untrusted": note("Brief", "2026-11-07T05:00:00Z", who="NONE"),
                 "not a brief": note("Thanks", "2026-11-07T05:00:00Z")}
        for why, comment in cases.items():
            with self.subTest(why):
                self.assertEqual(placed(FIRED, hub([pr(2, True)], {2: [comment]})), "running")

    def test_the_branch_names_the_issue_exactly(self):
        self.assertEqual(placed(FIRED, hub([pr(22, False), pr(2, True)])), "running")
        self.assertEqual(running.pulls_for(2, [pr(22, True), {"head": {"ref": "main"}}]), [])

    def test_what_cannot_be_read_is_unknown(self):
        self.assertEqual(placed(NO_TIME, hub()), "unknown")
        opened = {"state": "open"}
        blind = running.GitHub(pulls=[], comments=lambda n: None, issue=lambda n: opened, now=SOON,
                               closed=lambda n: [])
        self.assertEqual(placed(FIRED, blind), "unknown")
        gone = running.GitHub(pulls=[], comments=lambda n: [], issue=lambda n: None, now=SOON, closed=lambda n: [])
        self.assertEqual(placed(FIRED, gone), "unknown")
        unlisted = running.GitHub(pulls=[], comments=lambda n: [], issue=lambda n: opened, now=SOON,
                                  closed=lambda n: None)
        self.assertEqual(placed(FIRED, unlisted), "unknown")


class Coders(unittest.TestCase):
    def test_running_count_ended_does_not_and_unknown_is_set_apart(self):
        ready = {**FIRED, "name": "rusty #3 c"}
        stale = {**FIRED, "name": "rusty #4 d", "last_run": {"fired_at": "2026-11-07T01:00:00Z"}}
        counted, noted = running.coders([WAITING, FIRED, ready, stale, NO_TIME, {"name": "scorsese #2 b"}], 99,
                                        hub([pr(2, True), pr(3, False)], REVIEWED))
        self.assertEqual([c["name"] for c, _ in counted], ["rusty #1 a", "rusty #2 b"])
        self.assertEqual([(found["name"], placed) for found, placed, _ in noted],
                         [("rusty #4 d", "ended"), ("rusty #5 e", "unknown")])

    def test_only_the_newest_routine_of_an_issue_is_placed(self):
        old = {**WAITS, "created_at": "2026-11-06T00:00:00Z"}
        new = {**WAITING, "name": "rusty #2 b", "created_at": "2026-11-07T00:00:00Z"}
        counted, _ = running.coders([new, old], 99, hub([pr(2, False)], REVIEWED))
        self.assertEqual(counted, [(new, "not fired")])

    def test_the_issue_being_launched_is_never_counted(self):
        counted, _ = running.coders([WAITING, FIRED], 2, hub([pr(2, True)]))
        self.assertEqual(counted, [(WAITING, "not fired")])

    def test_only_rusty_routines_count(self):
        self.assertEqual(running.issue({"name": "scorsese #2 b"}), None)
        self.assertEqual(running.issue({"name": "rusty #1856 x"}), 1856)

    def test_the_description_says_when_it_fired_or_fires(self):
        self.assertIn("fired 2026-11-07T03:51Z", running.describe(FIRED))
        self.assertIn("fires 2026-11-07T04:28:00Z", running.describe(WAITING))


if __name__ == "__main__":
    unittest.main()
