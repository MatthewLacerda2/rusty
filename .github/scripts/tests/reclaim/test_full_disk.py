#!/usr/bin/env python3
"""#2436: reclaim runs on a full disk, freeing build output before any git write."""

from __future__ import annotations

import contextlib
import io
import subprocess
import unittest

from tests.reclaim.fixtures import MAIN, TREES, FakeMachine, reclaim

FULL = "fatal: Unable to create '.git/index.lock': No space left on device"
# The git subcommands that write; plain `status` refreshes the index, a write too.
WRITES = ("remove", "prune", "-D", "merge-tree", "fetch")


def writes(argv) -> bool:
    return argv[0] == "git" and (any(w in argv for w in WRITES)
                                 or ("status" in argv and "--no-optional-locks" not in argv))


class FullDisk(FakeMachine):
    """git refuses every write until `freed` build output was deleted."""

    def __init__(self, frees: bool):
        super().__init__()
        self.frees, self.freed = frees, False

    def answer(self, argv, **kwargs):
        if writes(argv) and not self.freed:
            self.calls.append(argv)
            return subprocess.CompletedProcess(argv, 128, "", FULL)
        return super().answer(argv, **kwargs)

    def drop_target(self, wt):
        super().drop_target(wt)
        self.freed = self.frees

    def remove(self, main, wt):
        self.git(main, "worktree", "remove", str(wt))
        super().remove(main, wt)


def run(machine) -> str:
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        reclaim.main([], machine, here=TREES / "open")
    return out.getvalue()


class Reclaim(unittest.TestCase):
    def test_build_output_goes_before_any_git_write_and_then_git_succeeds(self):
        machine = FullDisk(frees=True)
        out = run(machine)
        drop = machine.calls.index(["drop", "merged"])
        remove = machine.calls.index(["git", "-C", str(MAIN), "worktree", "remove", str(TREES / "merged")])
        self.assertLess(drop, remove, "the target/ is deleted before git is asked to write")
        self.assertEqual(machine.removed, ["merged"])
        self.assertIn("removed: merged (1-merged)", out)

    def test_judging_writes_nothing(self):
        machine = FullDisk(frees=False)
        with contextlib.suppress(SystemExit), contextlib.redirect_stdout(io.StringIO()):
            reclaim.main([], machine, here=TREES / "open")
        statuses = [argv for argv in machine.calls if "status" in argv]
        self.assertTrue(statuses)
        self.assertTrue(all("--no-optional-locks" in argv for argv in statuses))

    def test_a_git_write_refused_for_space_is_said_as_a_full_disk(self):
        machine = FullDisk(frees=False)
        with self.assertRaises(SystemExit) as stop, contextlib.redirect_stdout(io.StringIO()):
            reclaim.main([], machine, here=TREES / "open")
        said = str(stop.exception.code)
        self.assertTrue(said.startswith("reclaim: the disk is full (git worktree remove"), said)
        self.assertIn("`make reclaim` removes finished trees", said)
        self.assertIn(["drop", "merged"], machine.calls, "its build output was freed first")


if __name__ == "__main__":
    unittest.main()
