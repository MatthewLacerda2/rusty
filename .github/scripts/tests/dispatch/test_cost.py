"""#2452: `make coder-cost` measures each fired coder session, keeps it, and says the split."""

from __future__ import annotations

import contextlib
import io
import tempfile
import unittest
from pathlib import Path

from dispatch import cost, remote
from tests import load
from tests.dispatch.runlog_fixtures import NEWER, OLDER, STILL

cli = load("coder-cost.py")
URL = "https://api.github.com/repos/o/r/issues/"
BRIEF_URL = "https://github.com/o/r/issues/7#issuecomment-{}"


def routine(issue: int, session: str, comment: int, fired: str = "2026-11-20T02:27:10Z") -> dict:
    prompt = f"Read it.\nIssue(s): #{issue}. Your brief is the comment at {BRIEF_URL.format(comment)}; read it."
    return {"name": f"rusty #{issue} Do it", "created_at": fired,
            "last_run": {"session_id": session, "fired_at": fired, "status": "SUCCEEDED"},
            "job_config": {"ccr": {"events": [{"data": {"message": {"content": prompt}}}]}}}


def brief(ident: int, body: str, when: str, who: str = "OWNER") -> dict:
    return {"id": ident, "issue_url": f"{URL}7", "created_at": when, "body": body, "author_association": who}


THREAD = [brief(10, "Brief\n\nBranch: `7-x`.", "2026-11-19T00:00:00Z"),
          brief(11, "Brief\n\nRound: anything", "2026-11-19T06:00:00Z", who="NONE"),  # untrusted: no brief
          brief(12, "Brief\n\nRound: review\nBranch: `7-x`.", "2026-11-20T02:00:00Z")]
UNMOVED = {"status": "SUCCEEDED", "finished_at": None}
LOGS = {"cse_round": [NEWER, OLDER], "cse_busy": [STILL]}


class Api:
    """The routines API: one listing, and each session's log in pages of the given text."""

    def __init__(self, listed: list[dict]):
        self.listed, self.asked = listed, []

    def __call__(self, arguments: dict):
        self.asked.append(arguments)
        if arguments["action"] == "list":
            return 200, {"data": self.listed}, ""
        pages = LOGS.get(arguments["session_id"])
        if pages is None:
            return 404, None, ""
        page = int(arguments.get("cursor", "0"))
        more = str(page + 1) if page + 1 < len(pages) else None
        return 200, {"session_id": arguments["session_id"], "next_cursor": more}, pages[page]


class Measure(unittest.TestCase):
    def run_read(self, listed, kept=None):
        api = Api(listed)
        return cost.read(listed, kept or {}, api, lambda n: THREAD if n == 7 else None), api

    def test_a_round_is_measured_from_its_fire_through_every_page(self):
        done, api = self.run_read([routine(7, "cse_round", 12)])
        record = done.records["cse_round"]
        self.assertEqual((record["kind"], record["cause"]), ("round", "review"))
        self.assertEqual((record["init"], record["first_edit"], record["end"]), (48, 132, 2345))
        self.assertEqual((record["gates"], record["turns"]), (420, 46))
        self.assertEqual([a.get("cursor") for a in api.asked], [None, "1"])  # paged back to the start
        self.assertEqual(cost.split(record), {"sandbox": 48, "reading": 84, "gates": 420, "rest": 1793})

    def test_the_first_trusted_brief_is_the_first(self):
        done, _ = self.run_read([routine(7, "cse_round", 10)])
        self.assertEqual(done.records["cse_round"]["kind"], "first")

    def test_running_unreadable_kept_and_unfired_sessions_are_not_measured(self):
        unfired = {"name": "rusty #9 later", "enabled": True}
        other = {**routine(7, "cse_x", 10), "name": "scorsese #7 theirs"}
        listed = [routine(7, "cse_busy", 12), routine(8, "cse_gone", 12), routine(7, "cse_round", 12),
                  routine(7, "bad id!", 12), unfired, other]
        done, api = self.run_read(listed, kept={"cse_round": {"kind": "first", "listed": UNMOVED}})
        self.assertEqual(done.records, {})
        self.assertEqual(done.skipped, ["#7 cse_busy: still running", "#8 cse_gone: get_run_log answered HTTP 404",
                                        "#7: no plain session id listed"])
        self.assertNotIn("cse_round", [a.get("session_id") for a in api.asked])

    def test_a_brief_the_thread_does_not_hold_is_unknown(self):
        self.assertEqual(cost.kind(routine(7, "s", 99), lambda n: THREAD), ("?", "brief not found"))
        self.assertIsNone(cost.kind(routine(7, "s", 12), lambda n: None))  # GitHub did not answer


class Report(unittest.TestCase):
    RECORDS = {"a": {"issue": 7, "kind": "round", "cause": "review", "fired": "2026-11-20T02:27:10Z",
                     "init": 50, "first_edit": 150, "end": 1000, "gates": 600, "gates_reading": 0, "turns": 9},
               "b": {"issue": 8, "kind": "first", "cause": "", "fired": "2026-11-19T00:00:00Z",
                     "init": 100, "first_edit": None, "end": 400, "gates": 0, "gates_reading": 0, "turns": 2}}

    def test_it_says_the_listing_limit_the_tokens_and_the_split(self):
        told = cost.report(self.RECORDS, 1, ["#9 cse_q: still running"])
        self.assertIn("20 newest routines", told)
        self.assertIn("tokens are not in the run log", told)
        self.assertIn("skipped #9 cse_q: still running", told)
        self.assertIn("rounds: 1 sessions; cold start (sandbox + reading) median 15%", told)
        self.assertIn("sandbox 5%, reading 10%, gates 60%, rest 25%", told)
        self.assertIn("first briefs: 1 sessions; cold start (sandbox + reading) median 100%", told)  # no edit
        self.assertLess(told.index("#8 first"), told.index("#7 round (review)"))  # in fire order


class Command(unittest.TestCase):
    def test_it_keeps_what_it_measured_and_reads_it_back(self):
        with tempfile.TemporaryDirectory() as home:
            kept = Path(home, "coder-cost.json")
            api = Api([routine(7, "cse_round", 12)])
            gh = lambda path: THREAD if "page=1" in path else []  # noqa: E731
            for _ in range(2):
                with contextlib.redirect_stdout(io.StringIO()) as out:
                    self.assertEqual(cli.main([], gh=gh, ask=api, path=kept), 0)
            self.assertIn("1 sessions measured (0 read now", out.getvalue())
            self.assertEqual(sum(1 for a in api.asked if a["action"] == "get_run_log"), 2)  # first run only
            self.assertIn("cse_round", cost.load(kept))

    def test_a_listing_that_fails_exits_2(self):
        def down(arguments):
            raise remote.RemoteError("claude is not installed")
        with contextlib.redirect_stderr(io.StringIO()) as err:
            self.assertEqual(cli.main([], ask=down, path=Path("/nonexistent/x.json")), 2)
        self.assertIn("claude is not installed", err.getvalue())


if __name__ == "__main__":
    unittest.main()
