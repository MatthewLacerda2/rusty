#!/usr/bin/env python3
"""#1450 (review of PR #1451): a supervisor re-executing itself keeps a stop
request, and `kill <pid>` still releases its worktree lock.

[`daemon.supervised`] runs against a real git repository with a linked
worktree; only [`daemon.effects`] is faked, with a stop check that reads the
real stop file.
"""

from __future__ import annotations

import os
import signal
import tempfile
import unittest
import unittest.mock
from pathlib import Path

from merging.queue import world
from merging.watch import daemon, detach, ending, tenancy
from tests.watch.test_supervisor import fake
from tests.watch.test_tenancy import repo_with_worktree


def supervised(tree: str, restarted: bool, exits=(), watch=None) -> tuple[int, list[str], list[str]]:
    """Run the supervisor in `tree`: its status, what the effects said, and what it printed."""
    fx, said, _ = fake(list(exits))
    stop = daemon.common_dir(tree) / ending.STOP_FILE
    fx.stop_requested = lambda: world.consume(stop)
    fx.watch = watch or fx.watch
    with unittest.mock.patch.object(daemon, "effects", return_value=fx), unittest.mock.patch("builtins.print") as printed:
        status = daemon.supervised(tree, [], [], restarted=restarted)
    return status, said, [c.args[0] for c in printed.call_args_list]


def stop_for(tree: str) -> Path:
    stop = daemon.common_dir(tree) / ending.STOP_FILE
    stop.parent.mkdir(parents=True, exist_ok=True)
    stop.touch()
    return stop


class Restart(unittest.TestCase):
    def test_a_stop_written_before_a_restart_survives_and_ends_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            stop = stop_for(tree)
            status, said, printed = supervised(tree, restarted=True)
            self.assertEqual(status, ending.STOPPED_STATUS)
            self.assertIn("supervisor: a stop was requested; not starting the watch again.", said)
            self.assertNotIn(detach.CLEARED, printed)
            self.assertFalse(stop.exists())  # obeyed, by the supervisor
            self.assertIsNone(tenancy.current(tree))  # and its lock released

    def test_a_fresh_start_clears_a_stale_stop_and_runs_the_watch(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            stop_for(tree)
            status, said, printed = supervised(tree, restarted=False, exits=[(ending.STOPPED_STATUS, "")])
            self.assertEqual(status, ending.STOPPED_STATUS)
            self.assertIn(detach.CLEARED, printed)
            self.assertIn("supervisor: starting the watch.", said)

    def test_the_marker_is_what_a_restart_sets(self):
        with unittest.mock.patch("os.execve") as execve:
            fx = daemon.effects(".", [], ["--no-check"])
            fx.restart()
        self.assertEqual(execve.call_args.args[2].get(detach.RESTARTED), "1")
        self.assertNotIn(detach.RESTARTED, os.environ)


class Terminate(unittest.TestCase):
    def test_sigterm_mid_watch_releases_the_worktree_lock(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            locked = {}

            def watch():
                locked["during"] = tenancy.pid_of(tenancy.current(tree))
                os.kill(os.getpid(), signal.SIGTERM)
                return 0, ""
            def unhandled(*_):  # stands in for Python's default, which would end the test run
                raise AssertionError("SIGTERM reached the default handler")
            before = signal.signal(signal.SIGTERM, unhandled)
            self.addCleanup(signal.signal, signal.SIGTERM, before)
            with self.assertRaises(SystemExit) as ended:
                supervised(tree, restarted=False, watch=watch)
            self.assertEqual(ended.exception.code, 128 + signal.SIGTERM)
            self.assertEqual(locked["during"], os.getpid())
            self.assertIsNone(tenancy.current(tree))
            self.assertIs(signal.getsignal(signal.SIGTERM), unhandled)


if __name__ == "__main__":
    unittest.main()
