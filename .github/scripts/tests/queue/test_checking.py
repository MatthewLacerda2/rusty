#!/usr/bin/env python3
"""#571: a textually clean update is compiled before it is pushed."""

from __future__ import annotations

import subprocess
import unittest
from unittest import mock

from merging.queue import checks, cli, gitio, update
from tests.queue.fixtures import READY, completed
from tests.verdict.fixtures import SHA


class Checking(unittest.TestCase):
    """#571: a textually clean update is compiled before it is pushed."""

    def fake_git(self, merge=0, moved=True, unmerged=""):
        """`gitio.git` for [`update.advance`]: records every call, pushes included."""
        calls = []

        def git(*args, cwd=None):
            calls.append(args)
            if args[0] == "merge" and args[1] != "--abort":
                return completed(merge)
            if args[0] == "rev-parse":
                return subprocess.CompletedProcess([], 0, ("1" * 40 if moved else SHA) + "\n", "")
            if args[0] == "diff":
                return subprocess.CompletedProcess([], 0, unmerged, "")
            return completed(0)

        return git, calls

    def advance(self, check, **git):
        fake, calls = self.fake_git(**git)
        with mock.patch.object(gitio, "git", side_effect=fake):
            result = update.advance("b", SHA, ".", push=True, check=check)
        self.calls = calls
        return result, [c[0] for c in calls]

    def test_a_head_that_fails_the_check_is_never_pushed(self):
        (fresh, notes), verbs = self.advance(lambda work: ["the updated head fails cargo check (dev); not pushed."])
        self.assertIsNone(fresh)
        self.assertIn("not pushed", notes[0])
        self.assertNotIn("push", verbs)
        self.assertEqual(self.calls[-1][:2], ("worktree", "remove"))

    def test_a_head_that_passes_is_pushed(self):
        (fresh, notes), verbs = self.advance(lambda work: [])
        self.assertEqual((fresh, notes), ("1" * 40, []))
        self.assertIn("push", verbs)

    def test_a_conflict_is_handed_back_unchecked_and_unresolved(self):
        check = mock.Mock(return_value=[])
        (fresh, notes), verbs = self.advance(check, merge=1, unmerged="src/app/registry.rs\n")
        self.assertIsNone(fresh)
        self.assertIn("src/app/registry.rs", " ".join(notes))
        self.assertIn(("merge", "--abort"), self.calls)
        check.assert_not_called()
        self.assertNotIn("push", verbs)

    def test_an_update_that_moved_nothing_is_not_rechecked(self):
        check = mock.Mock(return_value=[])
        (fresh, _), verbs = self.advance(check, moved=False)
        self.assertEqual(fresh, SHA)
        check.assert_not_called()
        self.assertNotIn("push", verbs)

    def test_verify_runs_both_feature_sets_then_the_size_gate_in_the_updated_tree(self):
        runner = mock.Mock(return_value=completed(0))
        self.assertEqual(checks.verify("/tmp/wt", "/q/target", runner=runner), [])
        argvs = [c.args[0] for c in runner.call_args_list]
        self.assertEqual(argvs[0][:2], ["cargo", "check"])
        self.assertIn("--features", argvs[0])
        self.assertIn("--no-default-features", argvs[1])
        self.assertTrue(all("--locked" in a and "--all-targets" in a for a in argvs[:2]))
        self.assertIn("tools/lint/Cargo.toml", argvs[2])
        for call in runner.call_args_list:
            self.assertEqual(call.kwargs["cwd"], "/tmp/wt")
            self.assertEqual(call.kwargs["env"]["CARGO_TARGET_DIR"], "/q/target")

    def test_verify_stops_at_the_first_failure_and_quotes_it(self):
        broken = subprocess.CompletedProcess([], 101, "", "error[E0061]: this method takes 2 arguments but 3 were supplied")
        runner = mock.Mock(side_effect=[completed(0), broken])
        notes = checks.verify("/tmp/wt", "/q/target", runner=runner)
        self.assertEqual(runner.call_count, 2)
        self.assertIn("no default features", notes[0])
        self.assertIn("E0061", " ".join(notes))

    def test_what_is_checked_follows_the_pull_request_and_the_flags(self):
        code, docs = {**READY, "files": [{"path": "src/app/mod.rs"}]}, {**READY, "files": [{"path": "README.md"}]}
        self.assertTrue(checks.needs_check(code, cli.parse(["1"])))
        self.assertFalse(checks.needs_check(docs, cli.parse(["1"])))
        self.assertFalse(checks.needs_check(code, cli.parse(["1", "--no-check"])))
        self.assertFalse(checks.needs_check(code, cli.parse(["1", "--dry-run"])))


if __name__ == "__main__":
    unittest.main()
