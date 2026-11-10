#!/usr/bin/env python3
"""#1983: `queue-daemon.py` refuses the main working tree before forking, and a
supervisor that cannot lock its worktree stops instead of carrying on unlocked.

Both run against a real git repository with a linked worktree (as
`test_tenancy` does); the failed lock is git's own refusal to lock the main
working tree, the line the 2026-11-10 supervisor logged and ignored, and a
faked `git worktree lock` failing in a linked one.
"""

from __future__ import annotations

import contextlib
import io
import subprocess
import tempfile
import unittest
import unittest.mock
from pathlib import Path

from merging.queue import gitio
from merging.view import events
from merging.watch import daemon, supervisor as sv, tenancy
from tests.watch.test_tenancy import repo_with_worktree


def run_main(argv: list[str]) -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
        status = daemon.main(argv)
    return status, out.getvalue()


class MainTree(unittest.TestCase):
    def test_only_the_main_working_tree_is_main(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            self.assertTrue(tenancy.main_tree(tmp))
            self.assertTrue(tenancy.main_tree(str(Path(tmp, "queue", "..", "."))))
            self.assertFalse(tenancy.main_tree(tree))
        with tempfile.TemporaryDirectory() as bare:  # not a repository: not refused here
            self.assertFalse(tenancy.main_tree(bare))

    def test_the_main_tree_is_refused_before_forking_with_how_to_make_the_worktree(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_with_worktree(tmp)
            with unittest.mock.patch.object(daemon, "subprocess") as forked:
                status, out = run_main(["--detach", "--root", tmp, "--no-check"])
            forked.Popen.assert_not_called()
            self.assertEqual(status, sv.MAIN_TREE_STATUS)
            self.assertEqual(len(out.splitlines()), 1)
            self.assertIn("is the main working tree", out)
            top = gitio.git("rev-parse", "--show-toplevel", cwd=tmp).stdout.strip()
            self.assertIn(f"git -C {top} worktree add --detach {Path(top, tenancy.WORKTREE)} origin/main && cd ", out)
            self.assertIn("make queue-daemon ARGS=--no-check", out)

    def test_an_existing_queue_worktree_is_the_exact_command(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_with_worktree(tmp)
            tree = Path(tmp, tenancy.WORKTREE)
            subprocess.run(["git", "worktree", "add", "-q", "--detach", str(tree)], cwd=tmp, check=True)
            line = tenancy.refusal(tmp, ["--no-check", "--since", "2026-11-10T08:00"])
            top = gitio.git("rev-parse", "--show-toplevel", cwd=tmp).stdout.strip()
            self.assertIn(f"`cd {Path(top, tenancy.WORKTREE)} && make queue-daemon ARGS='--no-check --since 2026-11-10T08:00'`", line)
            self.assertEqual(tenancy.refusal(tmp, []).count("make queue-daemon`"), 1)  # no empty ARGS

    def test_a_stop_request_is_still_taken_from_the_main_tree(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_with_worktree(tmp)
            status, out = run_main(["--stop", "--root", tmp])
            self.assertEqual(status, 0)
            self.assertIn("asked the watch to stop", out)


class Unlocked(unittest.TestCase):
    def supervised(self, root: str) -> tuple[int, list[str]]:
        out = io.StringIO()
        with unittest.mock.patch.object(daemon, "effects") as effects, contextlib.redirect_stdout(out):
            status = daemon.supervised(root, [], [], restarted=True)
        effects.assert_not_called()  # stopped before noting code, so before any checkout move
        return status, out.getvalue().splitlines()

    def test_gits_refusal_to_lock_the_main_tree_stops_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_with_worktree(tmp)
            status, said = self.supervised(tmp)
            self.assertEqual(status, sv.UNLOCKED_STATUS)
            self.assertEqual(len(said), 1)
            self.assertRegex(said[0], r"^supervisor: could not lock .*main working tree.*; stopped\.$")
            self.assertTrue(events.keep(said[0], set()), said[0])  # `make queue-events` passes it

    def test_a_failed_lock_in_a_linked_worktree_stops_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            real = gitio.git

            def git(*args, cwd=None):
                if args[:2] == ("worktree", "lock"):
                    return subprocess.CompletedProcess(args, 128, "", "fatal: no space left on device")
                return real(*args, cwd=cwd)
            with unittest.mock.patch.object(gitio, "git", git):
                status, said = self.supervised(tree)
            self.assertEqual(status, sv.UNLOCKED_STATUS)
            self.assertIn("no space left on device", said[-1])
            self.assertIsNone(tenancy.current(tree))

    def test_another_lock_on_the_worktree_stops_it_and_stays(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            subprocess.run(["git", "worktree", "lock", "--reason", "claude agent", tree], cwd=tmp, check=True)
            status, said = self.supervised(tree)
            self.assertEqual(status, sv.UNLOCKED_STATUS)
            self.assertIn("already locked (claude agent)", said[-1])
            self.assertEqual(tenancy.current(tree), "claude agent")


if __name__ == "__main__":
    unittest.main()
