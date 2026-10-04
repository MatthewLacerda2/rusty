#!/usr/bin/env python3
"""#753: the watch takes the pull request that conflicts with the fewest others
in line, label priority as the tiebreak, and fails open when git cannot say.

The order is pure ([`queue_watch.pick`] with the conflicting pairs passed in);
[`queue_watch.clashing`] is driven with a fake git and once with a real one.
"""

from __future__ import annotations

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("queue_watch_select_c", HERE / "test_queue_watch_select.py")
select = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(select)
watch, pr, BOTS = select.watch, select.pr, select.BOTS
_spec = importlib.util.spec_from_file_location("queue_watch_loop_c", HERE / "test_queue_watch_loop.py")
loop = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(loop)

ISSUES = {1: {"infrastructure"}, 2: {"feature"}}
TREE = "a" * 40


def pick(pulls, clashes=(), memory=None):
    chosen = watch.pick(pulls, ISSUES, memory or {}, set(), BOTS, {frozenset(c) for c in clashes})
    return chosen and chosen["number"]


class Order(unittest.TestCase):
    def test_the_broad_one_goes_last_whatever_its_label(self):
        pulls = [pr(10, closes=[1]), pr(11, closes=[2]), pr(12, closes=[2])]
        self.assertEqual(pick(pulls, [(10, 11), (10, 12)]), 11)

    def test_equal_conflicts_fall_back_to_the_label_then_age(self):
        self.assertEqual(pick([pr(11, closes=[2]), pr(10, closes=[1])], [(10, 11)]), 10)
        self.assertEqual(pick([pr(21), pr(20)], [(20, 21)]), 20)

    def test_a_remembered_hand_back_counts_against_nobody(self):
        pulls = [pr(10, closes=[1], head="x"), pr(11, closes=[2]), pr(12, closes=[2])]
        self.assertEqual(pick(pulls, [(10, 11), (11, 12)], memory={12: f"sha{12}"}), 10)

    def test_dependabot_stays_last_with_no_conflict(self):
        self.assertEqual(pick([pr(5, bot=True), pr(6), pr(7)], [(6, 7)]), 6)

    def test_reordering_is_said_only_when_it_changes_who_goes_first(self):
        broad, small = pr(10, closes=[1]), pr(11, closes=[2])
        line = watch.reordered(small, broad, {10: 2, 11: 1})
        self.assertIn("#11 goes before #10", line)
        self.assertIn("#11 conflicts with 1 other(s) in line, #10 with 2", line)
        self.assertIsNone(watch.reordered(broad, broad, {}))


class FakeGit:
    """`merge-tree` answers from `verdicts` (by head pair); the rest succeed."""

    def __init__(self, verdicts, missing=()):
        self.verdicts, self.missing, self.calls = verdicts, set(missing), []

    def __call__(self, *args):
        self.calls.append(args)
        if args[0] == "cat-file":
            return (1 if args[2].split("^")[0] in self.missing else 0), ""
        if args[0] == "merge-tree":
            return self.verdicts.get(frozenset(args[2:]), (0, TREE + "\n"))
        return 0, ""


class Clashing(unittest.TestCase):
    def test_only_a_reported_conflict_counts(self):
        git = FakeGit({frozenset(("a", "b")): (1, f"{TREE}\nCONFLICT"), frozenset(("a", "c")): (1, "fatal: nope")})
        pulls = [pr(1, head="a"), pr(2, head="b"), pr(3, head="c")]
        self.assertEqual(watch.clashing(pulls, git, {}, BOTS), {frozenset((1, 2))})

    def test_git_that_cannot_run_fails_open(self):
        self.assertFalse(watch.conflicted(-1, ""))
        self.assertFalse(watch.conflicted(128, "fatal"))

    def test_known_heads_ask_git_nothing_and_missing_ones_are_fetched_once(self):
        git, known = FakeGit({}, missing=["b"]), {}
        pulls = [pr(1, head="a"), pr(2, head="b"), pr(3, head="c", bot=True)]
        watch.clashing(pulls, git, known, BOTS)
        self.assertIn(("fetch", "--quiet", "origin", "b"), git.calls)
        self.assertEqual(sum(1 for c in git.calls if c[0] == "merge-tree"), 1)
        git.calls.clear()
        watch.clashing(pulls, git, known, BOTS)
        self.assertEqual(git.calls, [])

    def test_a_real_merge_tree_conflict_is_read_as_one(self):
        with tempfile.TemporaryDirectory() as tmp:
            def run(*a):
                done = subprocess.run(["git", *a], cwd=tmp, capture_output=True, text=True, check=False)
                return done.returncode, done.stdout
            run("init", "-q", "-b", "main")
            run("config", "user.email", "t@t"), run("config", "user.name", "t")
            Path(tmp, "f.txt").write_text("x\n")
            run("add", "."), run("commit", "-q", "-m", "base")
            heads = {}
            for name, file in (("one", "f.txt"), ("two", "f.txt"), ("other", "g.txt")):
                run("checkout", "-q", "-b", name, "main")
                Path(tmp, file).write_text(f"{name}\n")
                run("add", "."), run("commit", "-q", "-m", name)
                heads[name] = run("rev-parse", "HEAD")[1].strip()
            pulls = [pr(1, head=heads["one"]), pr(2, head=heads["two"]), pr(3, head=heads["other"])]
            self.assertEqual(watch.clashing(pulls, run, {}, BOTS), {frozenset((1, 2))})


class InTheLoop(unittest.TestCase):
    def test_the_watch_takes_the_small_ones_first_and_says_why(self):
        with tempfile.TemporaryDirectory() as tmp:
            pulls = [pr(10, closes=[1]), pr(11, closes=[2]), pr(12, closes=[2])]
            world = loop.World(tmp, [pulls], dict.fromkeys((10, 11, 12), loop.queue.MERGED))
            world.clashes, said = {frozenset((10, 11)), frozenset((10, 12))}, []
            world.fx.issue_labels, world.fx.say = lambda: ISSUES, said.append
            watch.run(world.fx, loop.opts())
        # #11 first on the count; then #10 and #12 tie at one and the label decides.
        self.assertEqual(world.taken, [11, 10, 12])
        self.assertEqual(sum("goes before" in s for s in said), 1)
        self.assertIn("#11 goes before #10", next(s for s in said if "goes before" in s))


if __name__ == "__main__":
    unittest.main()
