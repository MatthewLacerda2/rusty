"""A machine for `reclaim.py` to judge: worktrees, their pull requests, and git's answers.

git and gh are faked by an injected runner that answers argv from tables, and
the disk effects (`free`, `remove`, `target_size`) are replaced, so nothing
here touches a real worktree.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import subprocess
import sys
from pathlib import Path

from tests import SCRIPTS

SCRIPT = SCRIPTS / "reclaim.py"
_spec = importlib.util.spec_from_file_location("reclaim", SCRIPT)
reclaim = importlib.util.module_from_spec(_spec)
sys.modules["reclaim"] = reclaim  # dataclasses look their module up while loading
_spec.loader.exec_module(reclaim)

Worktree, Facts = reclaim.Worktree, reclaim.Facts


MAIN = Path("/repo")

TREES = MAIN / ".claude" / "worktrees"

PORCELAIN = f"""worktree {MAIN}
HEAD aaaa
branch refs/heads/main

worktree {TREES}/merged
HEAD m111
branch refs/heads/1-merged

worktree {TREES}/open
HEAD o111
branch refs/heads/2-open

worktree {TREES}/dirty
HEAD d111
branch refs/heads/3-dirty

worktree {TREES}/agent
HEAD aaaa
branch refs/heads/worktree-agent
locked claude agent (pid 1)

worktree {TREES}/queue
HEAD q111
detached

worktree /elsewhere/merged-too
HEAD e111
branch refs/heads/4-elsewhere
"""

PRS = {"1-merged": "MERGED", "2-open": "OPEN", "3-dirty": "MERGED", "4-elsewhere": "MERGED"}


def rest_pull(state: str, head: str) -> dict:
    """A pull request as REST lists it, in GraphQL's `state` spelling."""
    merged = "2026-10-01T00:00:00Z" if state == "MERGED" else None
    return {"state": "open" if state == "OPEN" else "closed", "merged_at": merged, "head": {"sha": head}}


class FakeMachine(reclaim.Machine):
    """Answers git/gh from the tables above and records what it would delete."""

    def __init__(self):
        super().__init__(self.answer)
        self.removed: list[str] = []
        self.calls: list[list[str]] = []

    def answer(self, argv, **_):
        self.calls.append(argv)
        out, code = "", 0
        if argv[0] == "gh":
            branch = argv[2].split("head=o:")[1].split("&")[0]
            out = json.dumps([rest_pull(PRS[branch], "x")] if branch in PRS else [])
        elif argv[1:3] == ["remote", "get-url"]:
            out = "https://github.com/o/r.git"
        elif "--git-common-dir" in argv:
            out = f"{MAIN}/.git"
        elif argv[3:5] == ["worktree", "list"]:
            out = PORCELAIN
        elif argv[3:5] == ["status", "--porcelain"]:
            out = " M src/lib.rs" if argv[2].endswith("dirty") else ""
        elif "--is-ancestor" in argv:
            code = 0  # every detached HEAD here is on origin/main
        elif argv[3] == "rev-list":
            out = "0"
        return subprocess.CompletedProcess(argv, code, out, "")

    def cwds(self):
        return [(42, TREES / "queue" / "sub"), (7, Path("/tmp"))]

    def free(self, path):
        return 10 * 2**30

    def target_size(self, wt):
        return 3 * 2**30

    def remove(self, main, wt):
        self.removed.append(wt.name)


def run(*argv: str) -> tuple[FakeMachine, str]:
    machine, out = FakeMachine(), io.StringIO()
    with contextlib.redirect_stdout(out):
        reclaim.main(list(argv), machine, here=TREES / "open")
    return machine, out.getvalue()
