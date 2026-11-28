"""A bare repository standing in for GitHub, and two clones as the two sides of the lock.

The lock is plain git (`commit-tree`, a push that must fast-forward), so the
tests run the real commands against a local `origin`: nothing reaches GitHub.
"""

from __future__ import annotations

import contextlib
import subprocess
import tempfile
import unittest.mock
from datetime import datetime, timedelta, timezone
from pathlib import Path

from orchestrating import local, side



def no_lease():
    """The supervisor's tests run without the cross-machine lock: they have no `origin`."""
    return unittest.mock.patch.object(local, "held", lambda *_a, **_k: contextlib.nullcontext(None))


class Clock:
    """Starts at the real time: a side under test that reads the real clock (`local.held`,
    `orchestrator.py`) must see the fake holder's lease as fresh."""

    def __init__(self):
        self.now = datetime.now(timezone.utc).replace(microsecond=0)

    def __call__(self) -> datetime:
        return self.now

    def advance(self, minutes: float) -> None:
        self.now += timedelta(minutes=minutes)


class Remote:
    """`origin.git` and clones, each with its own state directory."""

    def __init__(self, case: unittest.TestCase):
        made = tempfile.TemporaryDirectory()
        case.addCleanup(made.cleanup)
        self.tmp = Path(made.name)
        self.origin = self.tmp / "origin.git"
        subprocess.run(["git", "init", "-q", "--bare", str(self.origin)], check=True)
        self.clock = Clock()

    def clone(self, name: str) -> str:
        root = self.tmp / name
        for args in (["init", "-q", str(root)], ["-C", str(root), "remote", "add", "origin", str(self.origin)],
                     ["-C", str(root), "config", "user.name", "t"], ["-C", str(root), "config", "user.email", "t@t"]):
            subprocess.run(["git", *args], check=True)
        return str(root)

    def side(self, name: str, me: str) -> side.Side:
        root = self.clone(name) if not (self.tmp / name).exists() else str(self.tmp / name)
        return side.Side(side.git_in(root), me, Path(root, ".git", "merge-queue"), self.clock)


CLOUD = "cloud cse_one"
OTHER_CLOUD = "cloud cse_two"
LOCAL = "local@desk queue-daemon"
