#!/usr/bin/env python3
"""#2436: a heavy build refuses before it starts on a disk without room for it."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from bench import space
from tests import SCRIPTS

GIB = space.GIB


def machine(sizes: dict[str, int]):
    """A root with trees under `.claude/worktrees/`, and a `du` answering `sizes` (path suffix -> bytes)."""
    root = Path(tempfile.mkdtemp())
    for suffix in sizes:
        (root / ".claude" / "worktrees" / suffix).mkdir(parents=True, exist_ok=True)

    def run(argv, **_):
        path = argv[-1]
        found = next((n for s, n in sizes.items() if path.endswith(s)), None)
        out = "" if found is None else f"{found // 1024}\t{path}\n"
        return subprocess.CompletedProcess(argv, 0 if found is not None else 1, out, "")
    return root, run


class Refusal(unittest.TestCase):
    def test_a_disk_under_the_need_refuses_naming_reclaim_and_the_largest_trees(self):
        root, run = machine({"pr-1-head/target": 11 * GIB, "bench-main/target": 2 * GIB})
        tree = root / ".claude" / "worktrees" / "pr-2-head"
        said = space.refusal(root, [(tree, "debug")], "test-gpu", run, free=lambda _: 3 * GIB)
        self.assertIsNotNone(said)
        self.assertIn("3.0 GiB free", said)
        self.assertIn("needs 14.0 GiB", said, "the whole debug build plus the headroom")
        self.assertIn("make reclaim", said)
        self.assertLess(said.index("pr-1-head 11.0 GiB"), said.index("bench-main 2.0 GiB"), "largest first")

    def test_a_disk_with_room_passes(self):
        root, run = machine({})
        tree = root / ".claude" / "worktrees" / "pr-2-head"
        self.assertIsNone(space.refusal(root, [(tree, "debug")], "test-gpu", run, free=lambda _: 15 * GIB))

    def test_a_warm_target_counts_as_built(self):
        root, run = machine({"pr-2-head/target/debug": 11 * GIB})
        tree = root / ".claude" / "worktrees" / "pr-2-head"
        self.assertIsNone(space.refusal(root, [(tree, "debug")], "test-gpu", run, free=lambda _: 3 * GIB))

    def test_two_builds_need_both(self):
        root, run = machine({})
        base = root / ".claude" / "worktrees"
        builds = [(base / "bench-main", "release"), (base / "pr-2-head", "release")]
        self.assertIsNone(space.refusal(root, builds[:1], "bench-ab", run, free=lambda _: 5 * GIB))
        said = space.refusal(root, builds, "bench-ab", run, free=lambda _: 5 * GIB)
        self.assertIn("needs 6.0 GiB", said)
        self.assertIn("bench-main/target/release 2.0 GiB, pr-2-head/target/release 2.0 GiB", said)


class Full(unittest.TestCase):
    def test_a_full_disk_is_told_apart_and_said_with_the_advice(self):
        root, run = machine({"pr-9-head/target": GIB})
        git = "fatal: sha1 file '.git/worktrees/x/index.lock' write error. Out of diskspace"
        self.assertTrue(space.said_full(git))
        self.assertTrue(space.said_full("error: No space left on device (os error 28)"))
        self.assertFalse(space.said_full("fatal: not a git repository"))
        said = space.full(git, root, run)
        self.assertTrue(said.startswith("the disk is full ("))
        self.assertIn("make reclaim", said)
        self.assertIn("pr-9-head 1.0 GiB", said)


class TestGpu(unittest.TestCase):
    def recipe(self, *args: str) -> str:
        return subprocess.run(["make", "-n", "--no-print-directory", "test-gpu", *args], cwd=SCRIPTS.parent.parent,
                              capture_output=True, text=True, check=True).stdout

    def test_a_green_pr_run_deletes_its_tree_s_debug_build_only_after_the_tests(self):
        line = self.recipe("PR=5")
        self.assertIn('cargo nextest run --locked --features dev -E \'group(gpu*)\' && rm -rf "$tree/target/debug"', line)
        self.assertNotIn("target/release", line.split("&& rm -rf")[1], "the bench's build stays")

    def test_a_local_run_deletes_nothing(self):
        self.assertNotIn("rm -rf", self.recipe())


if __name__ == "__main__":
    unittest.main()
