#!/usr/bin/env python3
"""#1956's review: what the [`lints.Linter`] remembers of a broken tool, and what it never does.

A tool that is broken is said once per `tools/lint` tree and left to CI; a
failure the machine caused (out of disk or memory, a killed process) is said
and retried on the next take, never cached as the tree's.
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from unittest import mock

from merging.queue import gitio, lints
from tests.queue.test_lints import tree_git


class Builds:
    """A `runner` whose `cargo build` answers `build` (a `CompletedProcess` or an
    exception) or writes a binary, and whose passes answer `passes`."""

    def __init__(self, build=None, passes=0):
        self.build, self.passes, self.builds, self.runs = build, passes, 0, 0

    def __call__(self, argv, **kwargs):
        if argv[0] == "cargo":
            self.builds += 1
            if isinstance(self.build, Exception):
                raise self.build
            if self.build is not None:
                return self.build
            exe = os.path.join(kwargs["env"]["CARGO_TARGET_DIR"], "release", lints.binary_name())
            os.makedirs(os.path.dirname(exe), exist_ok=True)
            open(exe, "w").close()
            return subprocess.CompletedProcess(argv, 0, "", "")
        self.runs += 1
        if isinstance(self.passes, Exception):
            raise self.passes
        return subprocess.CompletedProcess(argv, self.passes, "", "thread 'main' panicked")


class Memory(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.target = tmp.name
        patch = mock.patch.object(gitio, "git", side_effect=tree_git)
        patch.start()
        self.addCleanup(patch.stop)
        self.said: list[str] = []

    def linter(self, runner):
        return lints.Linter(self.target, runner=runner, say=self.said.append)

    def twice(self, runner):
        linter = self.linter(runner)
        self.assertEqual(linter.check("/wt", "h", "origin/main"), [])
        self.assertEqual(linter.check("/wt", "h", "origin/main"), [])
        return linter

    def test_a_full_disk_while_building_is_retried_on_the_next_take(self):
        full = subprocess.CompletedProcess([], 101, "", "error: No space left on device (os error 28)")
        runner = Builds(build=full)
        linter = self.twice(runner)
        self.assertEqual((runner.builds, linter.failed), (2, set()))
        self.assertIn("the next take tries again", self.said[0])

    def test_a_killed_build_is_the_machines_too(self):
        runner = Builds(build=subprocess.CompletedProcess([], -9, "", ""))
        self.twice(runner)
        self.assertEqual(runner.builds, 2)

    def test_a_crashing_pass_is_said_once_and_not_run_again_for_that_tree(self):
        runner = Builds(passes=101)
        linter = self.twice(runner)
        self.assertEqual((runner.builds, runner.runs, len(self.said)), (1, 1, 1))
        self.assertIn("crashed (exit 101", self.said[0])
        self.assertEqual(linter.crashed, {tree_git("rev-parse", "--verify", "--quiet").stdout.strip()})

    def test_a_pass_that_cannot_start_is_said_once(self):
        runner = Builds(passes=PermissionError("Permission denied"))
        self.twice(runner)
        self.assertEqual((runner.runs, len(self.said)), (1, 1))
        self.assertIn("could not run tools/lint", self.said[0])

    def test_a_pass_killed_by_the_machine_is_retried(self):
        runner = Builds(passes=-9)
        linter = self.twice(runner)
        self.assertEqual((runner.runs, linter.crashed, len(self.said)), (2, set(), 2))

    def test_a_relaunched_watch_finds_the_kept_binary_and_does_not_rebuild(self):
        first = Builds()
        self.assertEqual(self.linter(first).check("/wt", "h", "origin/main"), [])
        again = Builds()
        self.assertEqual(self.linter(again).check("/wt", "h", "origin/main"), [])
        self.assertEqual((first.builds, again.builds, again.runs), (1, 0, len(lints.PASSES)))


if __name__ == "__main__":
    unittest.main()
