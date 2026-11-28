"""#1569, #2382: the coder launch's fakes: GitHub and the routines API answering from tables,
`busy` and `drafts` for a board at the limit, and [`Board`] with several issues and their briefs."""

from __future__ import annotations

import contextlib
import io
import shutil
import tempfile
import weakref
from datetime import datetime, timezone
from pathlib import Path
from unittest import mock

from tests import load

coder = load("coder.py")
NOW = datetime(2026, 10, 29, 14, 3, tzinfo=timezone.utc)
BRIEF = {"body": "Brief\n\nBranch: `9-x`.", "created_at": "2026-10-29T10:00:00Z",
         "author_association": "OWNER", "html_url": "https://github.com/o/r/issues/9#issuecomment-501"}
OLDER = {**BRIEF, "created_at": "2026-10-28T10:00:00Z", "html_url": "https://github.com/o/r/issues/9#issuecomment-400"}
CHAT = {"body": "Thanks!", "created_at": "2026-10-29T11:00:00Z", "author_association": "NONE",
        "html_url": "https://github.com/o/r/issues/9#issuecomment-600"}
ROUTINES = {"data": [{"name": "rusty #8 earlier", "created_at": "2026-10-28T00:00:00Z",
                      "job_config": {"ccr": {"environment_id": "env_cloud"}}}]}


class Fake:
    """GitHub and the routines API, answering from tables and noting the calls."""

    def __init__(self, issue: dict | None = None, comments: list | None = None, create=(200, None),
                 routines: dict | None = None, pulls: list | None = None, threads: dict | None = None,
                 blockers: dict | None = None):
        # a pull request's conversation (#2017) and an issue's recorded `blocked_by` (#2063), by number
        self.threads, self.blockers = threads or {}, blockers or {}
        self.pulls = [] if pulls is None else pulls
        self.routines = ROUTINES if routines is None else routines
        self.issue = {"number": 9, "title": "Do it", "state": "open", "labels": []} if issue is None else issue
        self.comments = [BRIEF] if comments is None else comments
        self.create = create
        self.calls: list[dict] = []
        home = tempfile.mkdtemp()  # the waiting line (#2382), never this checkout's .git
        weakref.finalize(self, shutil.rmtree, home, True)
        self.line = Path(home, "merge-queue", "coders-waiting.json")

    def gh(self, path: str):
        if "/dependencies/blocked_by" in path:
            return self.blockers.get(int(path.split("/issues/")[1].split("/")[0]), [])
        if "/pulls" in path:
            return self.pulls if "page=1" in path else []
        if "/comments" in path:
            number = int(path.split("/issues/")[1].split("/")[0])
            return self.threads.get(number, self.comments) if "page=1" in path else []
        return self.issue

    def call(self, arguments: dict):
        self.calls.append(arguments)
        if arguments["action"] == "list":
            return 200, self.routines
        if isinstance(self.create, list):  # one answer per create, in turn; an exception is raised
            answer = self.create.pop(0)
            if isinstance(answer, Exception):
                raise answer
            status, answer = answer
        else:
            status, answer = self.create
        if status in (200, 201):  # the next listing has it, not fired, made after the ask
            made = {**arguments["body"], "enabled": True, "created_at": f"2026-10-29T15:{len(self.created()):02d}:00Z"}
            self.routines = {**self.routines, "data": [*self.routines["data"], made]}
        return status, answer if answer is not None else {"trigger": {"id": "trig_new"}}

    def run(self, *argv: str, env: dict | None = None) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.object(coder.ghrest, "slug", return_value="o/r"), \
                contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = coder.main(list(argv), gh=self.gh, call=self.call, now=NOW, env=env or {}, line=self.line)
        return code, out.getvalue(), err.getvalue()

    def created(self) -> list[dict]:
        return [c["body"] for c in self.calls if c["action"] == "create"]


def busy(*issues: int) -> dict:
    """A routine list with a fired coder on each of `issues`; its run reads finished (#1866)."""
    run = {"status": "ROUTINE_RUN_STATUS_SUCCEEDED", "fired_at": "2026-10-29T13:00:00Z",
           "finished_at": "2026-10-29T13:30:00Z"}
    return {"data": ROUTINES["data"] + [{"name": f"rusty #{n} x", "enabled": False, "ended_reason": "run_once_fired",
                                         "last_run": run} for n in issues]}


def drafts(*issues: int) -> list[dict]:
    return [{"draft": True, "head": {"ref": f"{n}-x"}} for n in issues]


def brief_on(number: int, at: str = "2026-10-29T10:00:00Z", comment: int = 1, body: str = "Brief\n\nx") -> dict:
    return {**BRIEF, "body": body, "created_at": at,
            "html_url": f"https://github.com/o/r/issues/{number}#issuecomment-{number}{comment}"}


ROUND = "Brief\nRound: review\n\nfix the findings"


class Board(Fake):
    """[`Fake`] answering each issue's own body and comments; `issues[n]` is (issue, comments)."""

    def __init__(self, issues: dict, **kwargs):
        super().__init__(**kwargs)
        self.issues = issues

    def open(self, number: int, comments: list | None = None, labels=(), state: str = "open") -> None:
        self.issues[number] = ({"number": number, "title": f"issue {number}", "state": state,
                                "labels": [{"name": n} for n in labels]}, [brief_on(number)] if comments is None else comments)

    def gh(self, path: str):
        if "/issues/" in path and "/dependencies/" not in path:
            number = int(path.split("/issues/")[1].split("/")[0].split("?")[0])
            if number in self.issues:
                issue, comments = self.issues[number]
                if "/comments" in path:
                    return comments if "page=1" in path else []
                return issue
        return super().gh(path)

    def launched(self) -> list[int]:
        return [int(b["name"].split("#")[1].split()[0]) for b in self.created()]

    def briefs(self) -> list[str]:
        return [b["job_config"]["ccr"]["events"][0]["data"]["message"]["content"] for b in self.created()]

    def share(self, ceiling: int, now) -> None:
        """The lock, for `--cap`: recorded, never pushed (`shared`; `unshared` makes it fail)."""
        if getattr(self, "unshared", None):
            raise self.unshared
        self.shared = [*getattr(self, "shared", []), ceiling]

    def coders(self, *argv: str) -> tuple[int, str]:
        """`make coders`, its stdout and stderr together."""
        cli, out = load("coders.py"), io.StringIO()
        with mock.patch.object(cli.ghrest, "slug", return_value="o/r"), contextlib.redirect_stdout(out), \
                contextlib.redirect_stderr(out):
            code = cli.main(list(argv), gh=self.gh, call=self.call, now=NOW, env={}, line=self.line,
                            shared=self.share)
        return code, out.getvalue()
