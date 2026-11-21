"""A faked `gh` for `blockers.py`: answers argv from a table, so the tests run offline.

The POSTs and DELETEs it sees are the whole of what `--fix` writes.
"""

from __future__ import annotations

import contextlib
import io
import json
import subprocess
from unittest import mock

from tests import load

blockers = load("blockers.py")

REPO = "o/r"


def issue(number: int, state: str = "open", **extra: object) -> dict:
    return {"number": number, "id": 9000 + number, "state": state, "title": f"t{number}"} | extra


class FakeGh:
    """Answers `gh` from `bodies` (the open issues listed), `issues` (fetched one by
    one, a blocker's own body included) and `recorded` (each issue's blocked_by)."""

    def __init__(self, bodies: dict[int, str], issues: list[dict], recorded=None, summary=None):
        self.bodies, self.recorded = bodies, recorded or {}
        self.issues = {i["number"]: i for i in issues}
        self.summary = summary or {}  # number -> open blocked_by count the listing reports
        self.posts: list[tuple[str, str]] = []
        self.deletes: list[str] = []
        self.asked: list[int] = []
        self.fetched: list[int] = []

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
                self.fetched.append(number)
                out = self.issues.get(number) or issue(number, body="")
        return subprocess.CompletedProcess(argv, 0, "" if out is None else json.dumps(out), "")

    def listed_summary(self, number: int) -> dict:
        if number not in self.summary:
            return {}
        open_count, total = self.summary[number] if isinstance(self.summary[number], tuple) \
            else (self.summary[number],) * 2
        return {"issue_dependencies_summary": {"blocked_by": open_count, "total_blocked_by": total}}


def run(fake: FakeGh, *argv: str) -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        with mock.patch.object(blockers.ghrest, "slug", return_value=REPO):
            code = blockers.main(list(argv), blockers.Gh(fake))
    return code, out.getvalue()
