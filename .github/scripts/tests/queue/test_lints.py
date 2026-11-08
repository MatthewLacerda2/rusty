#!/usr/bin/env python3
"""#1956: the lint pass on a merged tree, and how a broken local tool is carried past."""

from __future__ import annotations

import argparse
import os
import subprocess
import tempfile
import unittest
from unittest import mock

from merging.queue import checks, gitio, lints
from tests.queue.fixtures import READY
from tests.queue import test_taking


TREE = "c" * 40


def tree_git(*args, cwd=None):
    """`gitio.git` answering `HEAD:tools/lint` with [`TREE`], and an empty log."""
    if args[:3] == ("rev-parse", "--verify", "--quiet"):
        return subprocess.CompletedProcess([], 0, TREE + "\n", "")
    return subprocess.CompletedProcess([], 0, "", "")


class Bounded(unittest.TestCase):
    """A broken local tool is said once and never stops the queue."""

    def linter(self, runner):
        self.said: list[str] = []
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        patch = mock.patch.object(gitio, "git", side_effect=tree_git)
        patch.start()
        self.addCleanup(patch.stop)
        return lints.Linter(tmp.name, runner=runner, say=self.said.append)

    def test_a_failed_build_is_said_once_and_every_take_carries_on(self):
        runner = mock.Mock(return_value=subprocess.CompletedProcess([], 101, "", "error: could not compile `lint`"))
        linter = self.linter(runner)
        self.assertEqual(linter.check("/wt", "h", "origin/main"), [])
        self.assertEqual(linter.check("/wt", "h", "origin/main"), [])
        self.assertEqual(runner.call_count, 1)  # not rebuilt for the same tree
        self.assertEqual(len(self.said), 1)
        self.assertIn("could not compile", self.said[0])
        self.assertIn("left to CI", self.said[0])

    def test_no_cargo_is_a_build_failure_not_a_traceback(self):
        linter = self.linter(mock.Mock(side_effect=FileNotFoundError("cargo")))
        self.assertEqual(linter.check("/wt", "h", "origin/main"), [])
        self.assertEqual(len(self.said), 1)

    def test_a_pass_that_crashes_hands_nothing_back(self):
        def runner(argv, **kwargs):
            if argv[0] == "cargo":
                exe = os.path.join(kwargs["env"]["CARGO_TARGET_DIR"], "release", lints.binary_name())
                os.makedirs(os.path.dirname(exe), exist_ok=True)
                open(exe, "w").close()
                return subprocess.CompletedProcess(argv, 0, "", "")
            return subprocess.CompletedProcess(argv, 101, "", "thread 'main' panicked")

        linter = self.linter(runner)
        self.assertEqual(linter.check("/wt", "h", "origin/main"), [])
        self.assertIn("crashed (exit 101", self.said[0])

    def test_a_tree_without_tools_lint_is_left_alone(self):
        runner = mock.Mock()
        linter = lints.Linter("/t", runner=runner, say=mock.Mock())
        with mock.patch.object(gitio, "git", return_value=subprocess.CompletedProcess([], 1, "", "")):
            self.assertEqual(linter.check("/wt", "h", "origin/main"), [])
        runner.assert_not_called()


class Reading(unittest.TestCase):
    def test_the_pull_request_a_subject_names(self):
        self.assertEqual(lints.pull_of("Lightmap denoising (#1884) (#1924)"), "#1924")
        self.assertEqual(lints.pull_of("merge train: #1950 at abcdef123456"), "#1950")
        self.assertIsNone(lints.pull_of("Merge branch 'main' into b"))

    def test_the_report_drops_its_verdict_line(self):
        with tempfile.TemporaryDirectory() as work:
            os.makedirs(os.path.join(work, ".lint"))
            with open(os.path.join(work, lints.REPORT), "w") as out:
                out.write("lint: FAILED\nFILE_TOO_LONG src/app/play.rs 309/300\n\n")
            self.assertEqual(lints.report(work), ["FILE_TOO_LONG src/app/play.rs 309/300"])

    def test_every_lint_job_pass_runs_and_dead_pub_does_not(self):
        with open(os.path.join(os.path.dirname(__file__), "..", "..", "..", "workflows", "lint.yml")) as text:
            workflow = text.read()
        for flags in lints.PASSES:
            self.assertIn(" ".join(("tools/lint/Cargo.toml", *(("--", *flags) if flags else ()))), workflow)
        self.assertNotIn(("--dead-pub",), lints.PASSES)


class Which(unittest.TestCase):
    def opts(self, **kw):
        return argparse.Namespace(**{"no_check": False, "dry_run": False, **kw})

    def test_no_check_lints_and_never_compiles(self):
        pull = READY
        self.assertEqual((checks.needs_check(pull, self.opts()), checks.needs_lint(pull, self.opts())), (True, True))
        opts = self.opts(no_check=True)
        self.assertEqual((checks.needs_check(pull, opts), checks.needs_lint(pull, opts)), (False, True))
        self.assertFalse(checks.needs_lint(pull, self.opts(no_check=True, dry_run=True)))

    def test_a_markdown_only_branch_is_not_linted(self):
        pull = {**READY, "files": [{"path": "docs/notes.md"}]}
        self.assertFalse(checks.needs_lint(pull, self.opts(no_check=True)))


class Taken(unittest.TestCase):
    take = test_taking.Taking.take

    def test_a_no_check_take_lints_the_merged_tree_from_its_own_head(self):
        _, rebased, _, _ = self.take("--no-check")
        check = rebased.call_args.kwargs["check"]
        self.assertIsInstance(check.func.__self__, lints.Linter)
        self.assertEqual(check.keywords, {"since": READY["headRefOid"], "upto": "origin/main"})

    def test_a_checked_take_compiles_then_lints(self):
        _, rebased, _, _ = self.take()
        check = rebased.call_args.kwargs["check"]
        self.assertEqual((check.func, check.args[0].func), (lints.chained, checks.verify))
        self.assertIsInstance(check.args[1].func.__self__, lints.Linter)

    def test_both_stops_at_the_first_that_fails(self):
        second = mock.Mock(return_value=["lint"])
        self.assertEqual(lints.both(lambda w: ["compile"], second)("/wt"), ["compile"])
        second.assert_not_called()
        self.assertEqual(lints.both(lambda w: [], second)("/wt"), ["lint"])


if __name__ == "__main__":
    unittest.main()
