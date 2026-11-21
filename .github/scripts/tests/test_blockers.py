#!/usr/bin/env python3
"""#624: a blocker written only in prose is found, and `--fix` records it.

GitHub is faked by an injected runner that answers `gh` argv from a table, so
these run offline; the POSTs it sees are the whole of what `--fix` writes.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import subprocess
import unittest
from unittest import mock
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "blockers.py"
_spec = importlib.util.spec_from_file_location("blockers", SCRIPT)
blockers = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(blockers)

REPO = "o/r"


def issue(number: int, state: str = "open", **extra: object) -> dict:
    return {"number": number, "id": 9000 + number, "state": state, "title": f"t{number}"} | extra


class FakeGh:
    """Answers `gh` from `issues` (open ones listed with `bodies`) and `recorded`."""

    def __init__(self, bodies: dict[int, str], issues: list[dict], recorded=None, summary=None):
        self.bodies, self.recorded = bodies, recorded or {}
        self.issues = {i["number"]: i for i in issues}
        self.summary = summary or {}  # number -> blocked_by count the listing reports
        self.posts: list[tuple[str, str]] = []
        self.deletes: list[str] = []
        self.asked: list[int] = []

    def __call__(self, argv, **_):
        args = argv[1:]
        if "/issues?state=open" in args[-1]:
            listed = [{"number": n, "body": b} | self.listed_summary(n) for n, b in self.bodies.items()]
            out = listed + [{"number": 999, "body": "Blocked by #1", "pull_request": {}}]
        elif "POST" in args:
            self.posts.append((args[3], args[5]))
            out = {}
        elif "DELETE" in args:
            self.deletes.append(args[3])
            out = None
        else:
            path = args[1].split("?")[0].split("/")
            number = int(path[4])
            if path[-1] == "blocked_by":
                self.asked.append(number)
                out = self.recorded.get(number, [])
            else:
                out = self.issues[number]
        return subprocess.CompletedProcess(argv, 0, "" if out is None else json.dumps(out), "")

    def listed_summary(self, number: int) -> dict:
        if number not in self.summary:
            return {}
        count = self.summary[number]
        return {"issue_dependencies_summary": {"blocked_by": count, "total_blocked_by": count}}


def run(fake: FakeGh, *argv: str) -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        with mock.patch.object(blockers.ghrest, "slug", return_value=REPO):
            code = blockers.main(list(argv), blockers.Gh(fake))
    return code, out.getvalue()


class Mentions(unittest.TestCase):
    def test_reads_every_spelling_of_a_prose_blocker(self):
        cases = {
            "Blocked by #396": [396],
            "blocked BY #396, #397 and #398 & #399": [396, 397, 398, 399],
            "**Blocked by:** #12\n- blocked by #12": [12],
            "## Depends\nBlocked by #1. See also #2.": [1],
            # #399's own body, the incident behind #624.
            "**Blocked by #396** (materials must reference shaders first) and **#393** (naming).":
                [396, 393],
        }
        for body, want in cases.items():
            with self.subTest(body=body):
                self.assertEqual(blockers.mentioned(body), want)

    def test_ignores_negated_or_absent_blockers(self):
        for body in ("No longer blocked by #5", "not **blocked by** #5", "unblocked by #5",
                     "Blocks #5", "", None):
            with self.subTest(body=body):
                self.assertEqual(blockers.mentioned(body), [])


class Audit(unittest.TestCase):
    def fake(self) -> FakeGh:
        return FakeGh(
            bodies={
                399: "Blocked by #396 and #397",  # 396 open, 397 closed
                400: "Blocked by #396",  # already recorded
                401: "Blocked by #401 and #402",  # self, and a pull request
                402: "nothing here",
            },
            issues=[issue(396), issue(397, "closed"), issue(402, pull_request={})],
            recorded={400: [issue(396)]},
        )

    def test_reports_only_open_unrecorded_blockers_and_fails(self):
        fake = self.fake()
        code, out = run(fake)
        self.assertEqual(code, 1)
        self.assertIn("#399 blocked by #396 (t396): in the body, not recorded on GitHub", out)
        self.assertNotIn("#397", out)
        self.assertNotIn("#400 blocked", out)
        self.assertNotIn("#401 blocked", out)
        self.assertEqual(fake.posts, [], "an audit writes nothing")

    def test_fix_records_each_by_database_id(self):
        fake = self.fake()
        code, out = run(fake, "--fix")
        self.assertEqual(code, 0)
        self.assertEqual(fake.posts, [(f"repos/{REPO}/issues/399/dependencies/blocked_by",
                                       "issue_id=9396")])
        self.assertIn("#399 blocked by #396 (t396): recorded", out)

    def test_a_clean_board_passes(self):
        fake = FakeGh({1: "Blocked by #2"}, [issue(2)], {1: [issue(2)]})
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.posts), (0, []))
        self.assertIn("is recorded", out)

    def test_unknown_flags_are_refused(self):
        with self.assertRaises(SystemExit):
            blockers.main(["--fxi"], blockers.Gh(self.fake()))


class Stale(unittest.TestCase):
    """#2489: a recorded open blocker the body no longer states is stale."""

    def test_a_lifted_dependency_is_reported_and_fails(self):
        fake = FakeGh({5: "Was waiting on the cull; lifted, see the comment."}, [],
                      {5: [issue(7)]})
        code, out = run(fake)
        self.assertEqual(code, 1)
        self.assertIn("#5 blocked by #7 (t7): recorded on GitHub, not in the body", out)
        self.assertIn("1 stale", out)
        self.assertEqual((fake.posts, fake.deletes), ([], []), "an audit writes nothing")

    def test_fix_removes_it_by_database_id(self):
        fake = FakeGh({5: "No longer blocked by #7"}, [], {5: [issue(7)]})
        code, out = run(fake, "--fix")
        self.assertEqual(code, 0)
        self.assertEqual(fake.deletes, [f"repos/{REPO}/issues/5/dependencies/blocked_by/9007"])
        self.assertIn("#5 blocked by #7 (t7): removed", out)

    def test_agreeing_closed_or_reworded_blockers_pass(self):
        fake = FakeGh(
            bodies={
                1: "Blocked by #2",  # body and relationship agree
                3: "nothing",  # the recorded blocker is closed: history
                4: "**Blocked by:** **#8** (needs the bake) and #9",  # other accepted wording
            },
            issues=[],
            recorded={1: [issue(2)], 3: [issue(6, "closed")], 4: [issue(8), issue(9)]},
        )
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.posts, fake.deletes), (0, [], []))
        self.assertIn("every recorded one is in its body", out)

    def test_an_issue_listed_without_blockers_is_not_asked(self):
        fake = FakeGh({1: "plain", 2: "plain"}, [], {2: [issue(3)]}, summary={1: 0, 2: 1})
        code, _ = run(fake)
        self.assertEqual((code, fake.asked), (1, [2]))


if __name__ == "__main__":
    unittest.main()
