#!/usr/bin/env python3
"""Review of PR #1984 (#1983): the states the main-tree refusal and the claim
met after a power loss. A re-exec whose worktree is gone still ends with the
"is gone" line; an empty-reason lock (git writes it without an fsync) is
replaced, while a foreign one stops and names how to clear it; and the
refusal reads `git worktree list`, not the directory, for the command it prints.
"""

from __future__ import annotations

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from merging.watch import supervisor as sv, tenancy
from tests.watch.test_main_tree import run_main
from tests.watch.test_tenancy import repo_with_worktree


def git(*args: str, cwd: str) -> None:
    subprocess.run(["git", *args], cwd=cwd, capture_output=True, check=True)


class Gone(unittest.TestCase):
    def test_a_missing_root_is_the_gone_line_not_a_traceback(self):
        with tempfile.TemporaryDirectory() as tmp:
            gone = str(Path(tmp, "removed"))
            self.assertFalse(tenancy.main_tree(gone))
            status, out = run_main(["--root", gone])
            self.assertEqual(status, sv.GONE_STATUS)
            self.assertEqual(out.splitlines(), [f"supervisor: worktree {gone} is gone; stopped."])


class Locks(unittest.TestCase):
    def test_an_empty_reason_lock_is_replaced_and_said(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            git("worktree", "lock", tree, cwd=tmp)
            self.assertEqual(tenancy.current(tree), "")
            said: list[str] = []
            self.assertIsNone(tenancy.claim(tree, 4242, said.append))
            self.assertEqual(tenancy.pid_of(tenancy.current(tree)), 4242)
            self.assertEqual(len(said), 1)
            self.assertIn("no reason given", said[0])

    def test_a_foreign_lock_names_the_command_that_clears_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = repo_with_worktree(tmp)
            git("worktree", "lock", "--reason", "operator's", tree, cwd=tmp)
            why = tenancy.claim(tree, 4242)
            self.assertIn(f"`git worktree unlock {tree}`", why)
            self.assertEqual(tenancy.current(tree), "operator's")


class Refusal(unittest.TestCase):
    def follow(self, tmp: str) -> str:
        """The refusal's line, its command run (on `HEAD`, `true` for the daemon):
        a command that fails is no answer, so the worktree must exist after it."""
        line = tenancy.refusal(tmp, [])
        command = line.split("`")[1].replace("origin/main", "HEAD").replace("make queue-daemon", "true")
        subprocess.run(command, shell=True, cwd=tmp, capture_output=True, check=True)
        self.assertTrue(tenancy.registered(tmp, Path(tmp, tenancy.WORKTREE)))
        return line

    def test_a_plain_directory_is_not_offered_as_the_worktree(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_with_worktree(tmp)
            Path(tmp, tenancy.WORKTREE, "leftover").mkdir(parents=True)
            line = self.follow(tmp)
            self.assertNotIn("run `cd", line)
            self.assertIn("`mv ", line)

    def test_a_registered_worktree_whose_directory_is_gone_is_pruned_first(self):
        for locked in (False, True):
            with self.subTest(locked=locked), tempfile.TemporaryDirectory() as tmp:
                repo_with_worktree(tmp)
                tree = str(Path(tmp, tenancy.WORKTREE))
                git("worktree", "add", "-q", "--detach", tree, cwd=tmp)
                if locked:
                    git("worktree", "lock", tree, cwd=tmp)
                shutil.rmtree(tree)
                line = self.follow(tmp)
                self.assertIn("worktree prune && ", line)
                self.assertEqual("worktree unlock " in line, locked)


if __name__ == "__main__":
    unittest.main()
