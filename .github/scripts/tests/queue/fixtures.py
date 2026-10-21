"""A pull request as the queue reads it, and the helpers its tests share.

[`READY`] extends the verdict's fixture with the fields only the queue reads.
"""

from __future__ import annotations

import subprocess
import time
from unittest import mock

from merging.queue import gitio, polling, rebase
from tests.verdict import fixtures
from tests.verdict.fixtures import CODE, built


READY = {
    **fixtures.READY,
    "headRefName": "486-mergeable-queue",
    "state": "OPEN",
    "title": "Port make mergeable and make queue (#486)",
    "author": {"login": "MatthewLacerda2"},
}


def asked(*runs: dict, waited: float = 0.0, jobs=None, pull=None, files=CODE):
    """[`polling.progress`] over these runs, each a genuine build by default."""
    jobs = jobs if jobs is not None else {r["id"]: built(r["name"]) for r in runs}
    return polling.progress(pull or READY, list(runs), jobs, files, waited)


SPENT = polling.RUN_APPEARS_SECONDS + 1


def completed(code: int, stderr: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess([], code, "", stderr)


# [`rebase.advance`]'s push, with `git` faked (#875, #1108).
SHA, FRESH = "a" * 40, "1" * 40
STALE = " ! [rejected]        b -> b (stale info)\nerror: failed to push some refs"


def done(code: int, stderr: str = "", stdout: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess([], code, stdout, stderr)


def advancing(pushes, remote_head=FRESH, calls=None):
    """[`rebase.advance`] of branch `b` from [`SHA`], rebased to [`FRESH`].

    Each push answers with the next of `pushes`; `ls-remote` names
    `remote_head` (nothing when `None`). Every git call is appended to `calls`.
    """
    calls, pushes = ([] if calls is None else calls), list(pushes)

    def git(*args, cwd=None):
        calls.append(args)
        if args[0] == "rev-parse":
            return done(0, stdout=FRESH + "\n")
        if args[0] == "push":
            return pushes.pop(0)
        if args[0] == "ls-remote":
            return done(0, stdout=f"{remote_head}\trefs/heads/b\n" if remote_head else "")
        return done(0)

    with mock.patch.object(gitio, "git", side_effect=git), \
            mock.patch.object(rebase, "main_moved_by_docs", return_value=False), \
            mock.patch.object(time, "sleep"):
        return rebase.advance("b", SHA, ".", push=True)
