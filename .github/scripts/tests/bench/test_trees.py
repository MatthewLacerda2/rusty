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
        answers = {"origin/main": "aaa\n", "refs/bench/pr-42": "bbb\n"}
        git = lambda argv, **_: calls.append(argv) or answers.get(argv[-1], "")
        self.assertEqual(trees.fetch(42, git), ("aaa", "bbb"))
        self.assertIn(["git", "fetch", "--quiet", "origin", "+pull/42/head:refs/bench/pr-42"], calls)
        self.assertNotIn("FETCH_HEAD", [argv[-1] for argv in calls])

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
        env = trees.build_env({"CARGO_TARGET_DIR": "/shared", "CARGO_BUILD_TARGET_DIR": "/s2", "PATH": "/bin"})
        self.assertEqual(env, {"PATH": "/bin"})
        calls = []
        binary = trees.build(Path("/t/pr"), lambda argv, **_: calls.append(argv) or "")
        self.assertEqual(calls[0][-2:], ["--target-dir", "/t/pr/target"], "named to cargo, whatever a config says")
        self.assertEqual(binary, Path("/t/pr/target/release/bench-scenes"))


class StaleBase(unittest.TestCase):
    def test_the_pr_is_measured_with_main_merged_in(self):
        calls = []
        git = lambda argv, **_: calls.append(argv) or ("mmm\n" if "rev-parse" in argv else "")
        self.assertEqual(trees.merge_main(Path("/t/pr"), 7, "aaa", git), "mmm")
        merge = next(argv for argv in calls if "merge" in argv)
        self.assertEqual(merge[-1], "aaa")
        self.assertFalse(any("push" in argv for argv in calls), "never pushed")

    def test_a_conflict_aborts_and_says_to_update_the_branch(self):
        calls = []

        def git(argv, **_):
            calls.append(argv)
            if argv[-1] == "aaa":
                raise RuntimeError("CONFLICT (content)")
            return ""
        with self.assertRaisesRegex(RuntimeError, "update the branch"):
            trees.merge_main(Path("/t/pr"), 7, "aaa", git)
        self.assertEqual(calls[-1][-2:], ["merge", "--abort"])


def listing(tree: Path, locked: str | None) -> str:
    lock = "" if locked is None else f"\nlocked {locked}"
    return f"worktree /repo\nHEAD a\n\nworktree {tree}\nHEAD b\ndetached{lock}\n"


class Locks(unittest.TestCase):
    TREE = Path("/repo/.claude/worktrees/bench-main")

    def git(self, locked):
        calls = []
        answer = lambda argv, **_: calls.append(argv) or (listing(self.TREE, locked) if "list" in argv else "")
        return calls, answer

    def test_a_tree_is_locked_naming_the_run(self):
        calls, git = self.git(None)
        trees.lock(Path("/repo"), self.TREE, 42, git, alive=lambda pid: True)
        self.assertIn(["git", "-C", "/repo", "worktree", "lock", "--reason", "bench-ab pid 42", str(self.TREE)], calls)
        self.assertEqual(trees.lock_pid_of("bench-ab pid 42"), 42)
        self.assertIsNone(trees.lock_pid_of("claude agent agent-x (pid 9)"))

    def test_a_dead_runs_lock_is_taken_over_and_a_live_or_foreign_one_refuses(self):
        calls, git = self.git("bench-ab pid 7")
        trees.lock(Path("/repo"), self.TREE, 42, git, alive=lambda pid: False)
        self.assertIn(["git", "-C", "/repo", "worktree", "unlock", str(self.TREE)], calls)
        for held, alive in (("bench-ab pid 7", True), ("someone else", False)):
            _, git = self.git(held)
            with self.assertRaisesRegex(RuntimeError, "not ours"):
                trees.lock(Path("/repo"), self.TREE, 42, git, alive=lambda pid: alive)

    def test_unlock_drops_only_its_own(self):
        calls, git = self.git("bench-ab pid 7")
        trees.unlock(Path("/repo"), self.TREE, 42, git)
        self.assertFalse(any("unlock" in argv for argv in calls))
        trees.unlock(Path("/repo"), self.TREE, 7, git)
        self.assertTrue(any("unlock" in argv for argv in calls))


if __name__ == "__main__":
    unittest.main()
