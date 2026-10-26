#!/usr/bin/env python3
"""#1453: a merged branch the queue rebased is excused when its content is in origin/main.

A real scratch repo answers `merge-tree`; only the version is faked.
"""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from tests.reclaim.fixtures import reclaim

ENV = ["-c", "user.name=t", "-c", "user.email=t@t"]


def git(cwd, *args):
    return subprocess.run(["git", *ENV, "-C", str(cwd), *args], check=True, capture_output=True, text=True).stdout


class Rebased(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.repo = Path(tmp.name)
        git(self.repo, "init", "-q", "-b", "main")
        (self.repo / "a").write_text("1\n")
        git(self.repo, "add", "a")
        git(self.repo, "commit", "-qm", "base")
        git(self.repo, "checkout", "-qb", "work")
        (self.repo / "b").write_text("b\n")
        git(self.repo, "add", "b")
        git(self.repo, "commit", "-qm", "work")
        # main gets the squash of the work (a different commit) plus unrelated progress
        git(self.repo, "checkout", "-q", "main")
        (self.repo / "b").write_text("b\n")
        (self.repo / "c").write_text("c\n")
        git(self.repo, "add", "b", "c")
        git(self.repo, "commit", "-qm", "squash")
        git(self.repo, "update-ref", "refs/remotes/origin/main", "main")
        git(self.repo, "checkout", "-q", "work")

    def machine(self, version="git version 2.43.0"):
        def runner(argv, **kw):
            if argv[:2] == ["git", "--version"]:
                return subprocess.CompletedProcess(argv, 0, version + "\n", "")
            return subprocess.run(argv, **kw)
        return reclaim.Machine(runner)

    def test_rebased_and_squashed_into_main_is_contained(self):
        self.assertTrue(self.machine().merged_into_main(self.repo))

    def test_an_extra_local_commit_is_not_contained(self):
        (self.repo / "a").write_text("2\n")
        git(self.repo, "commit", "-qam", "extra")
        self.assertFalse(self.machine().merged_into_main(self.repo))

    def test_an_old_git_falls_back_and_says_so(self):
        machine = self.machine("git version 2.30.2")
        self.assertFalse(machine.merged_into_main(self.repo))
        self.assertTrue(machine.old_git)


if __name__ == "__main__":
    unittest.main()
