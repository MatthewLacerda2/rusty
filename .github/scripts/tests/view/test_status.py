"""#1876: liveness is the supervisor's worktree lock's PID, checked alive, never a
command-line match. Runs against a real git repository with a linked worktree."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

from merging.view import status
from merging.watch import tenancy
from tests.watch.test_tenancy import repo_with_worktree


def dead_pid() -> int:
    """A PID that was just alive and is not any more."""
    child = subprocess.Popen(["true"])
    child.wait()
    return child.pid


class Read(unittest.TestCase):
    def test_a_lock_whose_pid_is_dead_says_not_running(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            pid = dead_pid()
            tenancy.claim(tree, pid)
            state = status.read(tree)
            self.assertFalse(state.running)
            self.assertIn(f"PID {pid}, which is not a live supervisor", state.lines[0])

    def test_a_lock_whose_pid_is_a_live_supervisor_says_running(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            tenancy.claim(tree, os.getpid())
            state = status.read(tree, check=lambda pid: pid == os.getpid())
            self.assertTrue(state.running)
            self.assertIn(f"supervisor running, PID {os.getpid()}", state.lines[0])

    def test_no_lock_says_not_running(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertFalse(status.read(repo_with_worktree(tmp)).running)

    def test_another_lock_is_not_a_supervisor(self):
        listing = "worktree /a\nHEAD 1\ndetached\nlocked claude agent agent-x (pid 12 start 3)\n"
        self.assertEqual(status.locks(listing), [])

    def test_one_live_supervisor_of_two_locks_is_running(self):
        held = [("/a", 1), ("/b", 2)]
        state = status.judge(held, lambda pid: pid == 2)
        self.assertEqual(state, status.Status(True, ["queue-status: supervisor running, PID 2, in /b."]))


class Identity(unittest.TestCase):
    def test_a_live_pid_running_something_else_is_not_a_supervisor(self):
        # A reboot hands PIDs out again: the lock outlives the process it named.
        self.assertFalse(status.is_supervisor(7, alive=lambda _: True, ask=lambda _: "/usr/bin/vim notes"))
        self.assertTrue(status.is_supervisor(7, alive=lambda _: True, ask=lambda _: "python3 .github/scripts/queue-daemon.py"))
        self.assertTrue(status.is_supervisor(7, alive=lambda _: True, ask=lambda _: None))
        self.assertFalse(status.is_supervisor(7, alive=lambda _: False, ask=lambda _: "queue-daemon.py"))

    def test_this_process_is_not_a_supervisor(self):
        self.assertFalse(status.is_supervisor(os.getpid()))

    def test_an_exported_columns_does_not_cut_the_command_line(self):
        tail = "queue-daemon.py"
        child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)", "x" * 120 + tail])
        try:
            with mock.patch.dict(os.environ, {"COLUMNS": "20"}):
                command, until = "", time.monotonic() + 10
                while not command.endswith(tail) and time.monotonic() < until:  # a fork shows its parent's line until it execs
                    command = status.identity(child.pid) or ""
                self.assertTrue(command.endswith(tail), command)
        finally:
            child.kill()
            child.wait()


class LastLine(unittest.TestCase):
    def test_says_the_last_line_and_its_age(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp, "queue.log")
            log.write_text("queue: #1: merged.\nsupervisor: starting the watch.\n\n")
            said = status.last_line(log, now=time.time() + 600)
            self.assertIn("10 min ago: supervisor: starting the watch.", said)
            self.assertIn("no log at", status.last_line(Path(tmp, "missing"), now=0))


if __name__ == "__main__":
    unittest.main()
