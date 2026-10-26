#!/usr/bin/env python3
"""#1450: the supervisor locks its worktree against `make reclaim`, and ends with
a line, not a traceback, when the worktree is gone anyway.

The lock runs against a real git repository with a linked worktree; the ending
drives [`supervisor.supervise`] with the fake effects of `test_supervisor`.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from merging.watch import supervisor as sv, tenancy
from tests.watch.test_supervisor import fake


def repo_with_worktree(tmp: str) -> str:
    git = lambda *a, cwd=tmp: subprocess.run(["git", *a], cwd=cwd, capture_output=True, check=True)  # noqa: E731
    git("init", "-q", "-b", "main"), git("config", "user.email", "t@t"), git("config", "user.name", "t")
    git("commit", "-q", "--allow-empty", "-m", "one")
    tree = str(Path(tmp, "queue"))
    git("worktree", "add", "-q", "--detach", tree)
    return tree


class Lock(unittest.TestCase):
    def test_claim_locks_with_the_pid_and_release_unlocks(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            self.assertIsNone(tenancy.current(tree))
            self.assertIsNone(tenancy.claim(tree, 4242))
            self.assertEqual(tenancy.pid_of(tenancy.current(tree)), 4242)
            tenancy.release(tree, 1)  # another PID's release leaves it
            self.assertEqual(tenancy.pid_of(tenancy.current(tree)), 4242)
            self.assertIsNone(tenancy.claim(tree, 4343))  # a dead supervisor's lock is replaced
            self.assertEqual(tenancy.pid_of(tenancy.current(tree)), 4343)
            tenancy.release(tree, 4343)
            self.assertIsNone(tenancy.current(tree))

    def test_another_lock_is_left_and_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            subprocess.run(["git", "worktree", "lock", "--reason", "claude agent", tree], cwd=tmp, check=True)
            self.assertIn("already locked (claude agent)", tenancy.claim(tree, 4242))
            self.assertEqual(tenancy.current(tree), "claude agent")

    def test_pid_of_reads_only_a_supervisor_reason(self):
        self.assertEqual(tenancy.pid_of(tenancy.reason(7)), 7)
        for other in (None, "", "claude agent (pid 1)", "merge-queue supervisor pid x"):
            self.assertIsNone(tenancy.pid_of(other))
        self.assertTrue(tenancy.alive(os.getpid()))


def gone_after(fx, watches: int):
    """Make `fx.gone()` report its worktree once `watches` watches have run."""
    counted = {"n": 0}
    watch = fx.watch

    def counting():
        counted["n"] += 1
        return watch()
    fx.watch = counting
    fx.gone = lambda: "/w/queue" if counted["n"] >= watches else None
    return fx


def supervise(fx) -> int:
    return sv.supervise(lambda: fx, lambda: fx.gone(), fx.say)


class Gone(unittest.TestCase):
    def test_a_worktree_removed_mid_wait_ends_it_with_its_line(self):
        fx, said, calls = fake([(0, "no open pull request is left.")])
        fx = gone_after(fx, 1)
        self.assertEqual(supervise(fx), sv.GONE_STATUS)
        self.assertEqual(said[-1], "supervisor: worktree /w/queue is gone; stopped.")
        self.assertLess(calls["slept"], sv.PAUSE_SECONDS)

    def test_a_present_worktree_keeps_it_running(self):
        fx, said, calls = fake([(0, "no open pull request is left."), (6, "")])
        fx = gone_after(fx, 99)
        self.assertEqual(supervise(fx), 6)
        self.assertEqual(calls["slept"], sv.PAUSE_SECONDS)

    def test_git_failing_in_a_removed_worktree_is_the_gone_line_not_a_traceback(self):
        fx, said, _ = fake([])
        state = {"gone": None}

        def update():
            state["gone"] = "/w/queue"
            raise FileNotFoundError(2, "No such file or directory", "/w/queue")
        fx.update, fx.gone = update, lambda: state["gone"]
        self.assertEqual(supervise(fx), sv.GONE_STATUS)
        self.assertIn("is gone; stopped.", said[-1])

    def test_a_worktree_gone_before_the_effects_exist_is_the_gone_line(self):
        said = []

        def begin():  # a restart into a removed worktree: the lock's git call fails first
            raise FileNotFoundError(2, "No such file or directory", "/w/queue")
        self.assertEqual(sv.supervise(begin, lambda: "/w/queue", said.append), sv.GONE_STATUS)
        self.assertEqual(said, ["supervisor: worktree /w/queue is gone; stopped."])

    def test_anything_else_unexpected_is_one_crashed_line(self):
        fx, said, _ = fake([])
        fx.gone = lambda: None
        fx.update = lambda: {}["missing"]
        self.assertEqual(supervise(fx), sv.CRASHED_STATUS)
        self.assertEqual(said[-1], "supervisor: crashed: KeyError: 'missing'; stopped.")

    def test_release_after_a_real_removal_does_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            shutil.rmtree(tree)
            self.assertFalse(os.path.isdir(tree))
            tenancy.release(tree, 1)  # nothing to unlock, and no FileNotFoundError


if __name__ == "__main__":
    unittest.main()
