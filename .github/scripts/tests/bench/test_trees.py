#!/usr/bin/env python3
"""#2381: a PR's head is fetched as pull/<n>/head and checked out detached under .claude/worktrees/."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from bench import trees


class Trees(unittest.TestCase):
    def test_the_head_is_fetched_by_pull_ref_never_a_branch(self):
        calls = []
        answers = {"origin/main": "aaa\n", "FETCH_HEAD": "bbb\n"}
        git = lambda argv, **_: calls.append(argv) or answers.get(argv[-1], "")
        self.assertEqual(trees.fetch(42, git), ("aaa", "bbb"))
        self.assertIn(["git", "fetch", "--quiet", "origin", "pull/42/head"], calls)

    def test_a_new_tree_is_added_detached_and_an_old_one_re_pointed(self):
        root, calls = Path(tempfile.mkdtemp()), []
        git = lambda argv, **_: calls.append(argv) or ""
        tree = trees.checkout(root, trees.pr_tree(7), "bbb", git)
        self.assertEqual(tree, root / ".claude" / "worktrees" / "pr-7-head")
        self.assertEqual(calls[-1][-3:], ["--detach", str(tree), "bbb"])
        self.assertIn("worktree", calls[-1])
        self.assertIn("--detach", calls[-1])
        (tree / ".git").parent.mkdir(parents=True)
        (tree / ".git").write_text("gitdir: x")
        trees.checkout(root, trees.pr_tree(7), "ccc", git)
        self.assertEqual(calls[-1], ["git", "-C", str(tree), "checkout", "--quiet", "--detach", "ccc"])

    def test_a_build_never_shares_a_target_dir(self):
        self.assertNotIn("CARGO_TARGET_DIR", trees.build_env({"CARGO_TARGET_DIR": "/shared", "PATH": "/bin"}))


if __name__ == "__main__":
    unittest.main()
