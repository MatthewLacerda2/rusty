#!/usr/bin/env python3
"""#2381: a tree is locked before it is re-pointed, a run puts back only what it
locked, and a lock a power cut left on a reused PID does not block every later run."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from bench import trees


class Repo:
    """A main checkout whose `.claude/worktrees/<name>` exists (or not), with git faked."""

    def __init__(self, name: str, exists: bool, locked: str | None):
        self.root = Path(tempfile.mkdtemp())
        self.tree = trees.path(self.root, name)
        if exists:
            self.tree.mkdir(parents=True)
            (self.tree / ".git").write_text("gitdir: x")
        self.locked, self.calls = locked, []

    def git(self, argv, **_):
        self.calls.append(argv)
        if "list" in argv:
            lock = "" if self.locked is None else f"\nlocked {self.locked}"
            return f"worktree {self.root}\nHEAD a\n\nworktree {self.tree}\nHEAD b\ndetached{lock}\n"
        return ""

    def verbs(self):
        return [next(w for w in ("checkout", "add", "unlock", "lock") if w in argv)
                for argv in self.calls if "list" not in argv]


class Take(unittest.TestCase):
    def test_an_existing_tree_is_locked_before_it_is_re_pointed(self):
        repo, locked = Repo("bench-main", True, None), []
        trees.take(repo.root, "bench-main", "aaa", 42, locked, repo.git, alive=lambda pid: True)
        self.assertEqual(repo.verbs(), ["lock", "checkout"])
        self.assertEqual(locked, [repo.tree])

    def test_a_new_tree_is_added_then_locked(self):
        repo, locked = Repo("pr-7-head", False, None), []
        trees.take(repo.root, "pr-7-head", "bbb", 42, locked, repo.git, alive=lambda pid: True)
        self.assertEqual(repo.verbs(), ["add", "lock"])

    def test_a_live_runs_tree_is_never_re_pointed_nor_put_back(self):
        repo, locked = Repo("pr-7-head", True, "bench-ab pid 7"), []
        with self.assertRaisesRegex(RuntimeError, "git worktree unlock"):
            trees.take(repo.root, "pr-7-head", "bbb", 42, locked, repo.git, alive=lambda pid: True)
        self.assertNotIn("checkout", repo.verbs(), "the live run's sources stay put")
        self.assertEqual(locked, [], "so `finally` neither restores nor unlocks it")


class PowerCut(unittest.TestCase):
    def test_holding_the_flock_takes_over_a_reused_pids_lock(self):
        repo, locked = Repo("bench-main", True, "bench-ab pid 7"), []
        trees.take(repo.root, "bench-main", "aaa", 42, locked, repo.git, alive=lambda pid: True, exclusive=True)
        self.assertEqual(repo.verbs(), ["unlock", "lock", "checkout"])

    def test_a_foreign_lock_refuses_even_holding_the_flock(self):
        repo = Repo("bench-main", True, "claude agent agent-x (pid 9)")
        with self.assertRaisesRegex(RuntimeError, "not ours"):
            trees.lock(repo.root, repo.tree, 42, repo.git, alive=lambda pid: False, exclusive=True)


class PrTree(unittest.TestCase):
    def test_a_live_runs_tree_is_named_and_a_dead_ones_is_not(self):
        repo = Repo("pr-7-head", True, "bench-ab pid 7")
        self.assertEqual(trees.held_by_run(repo.root, repo.tree, repo.git, alive=lambda pid: True), 7)
        self.assertIsNone(trees.held_by_run(repo.root, repo.tree, repo.git, alive=lambda pid: False))
        self.assertIsNone(trees.held_by_run(Repo("pr-8-head", False, None).root, Path("/none"), repo.git))

    def test_a_tree_name_carries_its_pr(self):
        self.assertEqual(trees.pr_of_tree(trees.pr_tree(2398)), 2398)
        self.assertIsNone(trees.pr_of_tree(trees.MAIN_TREE))
        self.assertIsNone(trees.pr_of_tree("pr-x-head"))


if __name__ == "__main__":
    unittest.main()
