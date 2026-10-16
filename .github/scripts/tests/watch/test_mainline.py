#!/usr/bin/env python3
"""#928: a ready pull request that conflicts with `main` is handed back at once,
naming the files, instead of waiting in line to find out.

[`mainline.conflicts`] is driven with a fake git and once with a real one; the
loop with the fake world.
"""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from merging.queue import outcomes
from merging.watch import ending, handbacks, loop, mainline
from tests.watch import fakes
from tests.watch.test_select import BOTS, pr


MAIN, TREE = "m" * 40, "a" * 40


def fake_git(answers):
    calls = []

    def git(*args):
        calls.append(args)
        if args[0] == "rev-parse":
            return 0, MAIN + "\n"
        if args[0] == "merge-tree":
            return answers.get(args[-1], (0, TREE + "\n"))
        return 0, ""
    return git, calls


class Conflicts(unittest.TestCase):
    def test_names_the_conflicted_files_once_each(self):
        git, _ = fake_git({"b": (1, f"{TREE}\nsrc/x.rs\nsrc/x.rs\nMakefile\n")})
        self.assertEqual(mainline.conflicts([pr(1, head="a"), pr(2, head="b")], git, {}, BOTS), {2: ["Makefile", "src/x.rs"]})

    def test_a_known_pair_asks_git_no_merge_and_dependabot_is_left_out(self):
        git, calls = fake_git({})
        known = {}
        mainline.conflicts([pr(1, head="a"), pr(2, head="b", bot=True)], git, known, BOTS)
        mainline.conflicts([pr(1, head="a")], git, known, BOTS)
        self.assertEqual([c[-1] for c in calls if c[0] == "merge-tree"], ["a"])

    def test_git_that_cannot_say_fails_open(self):
        def broken(*args):
            return (128, "fatal") if args[0] in ("rev-parse", "merge-tree") else (0, "")
        self.assertEqual(mainline.conflicts([pr(1, head="a")], broken, {}, BOTS), {})

    def test_a_real_conflict_with_main(self):
        with tempfile.TemporaryDirectory() as tmp:
            def run(*a):
                done = subprocess.run(["git", *a], cwd=tmp, capture_output=True, text=True, check=False)
                return done.returncode, done.stdout
            run("init", "-q", "-b", "main")
            run("config", "user.email", "t@t"), run("config", "user.name", "t")
            Path(tmp, "f.txt").write_text("x\n")
            run("add", "."), run("commit", "-q", "-m", "base")
            heads = {}
            for name, file in (("clash", "f.txt"), ("clean", "g.txt")):
                run("checkout", "-q", "-b", name, "main")
                Path(tmp, file).write_text(f"{name}\n")
                run("add", "."), run("commit", "-q", "-m", name)
                heads[name] = run("rev-parse", "HEAD")[1].strip()
            run("checkout", "-q", "main")
            Path(tmp, "f.txt").write_text("main\n")
            run("commit", "-q", "-am", "main moves")
            run("update-ref", "refs/remotes/origin/main", "main")
            pulls = [pr(1, head=heads["clash"]), pr(2, head=heads["clean"])]
            self.assertEqual(mainline.conflicts(pulls, run, {}, BOTS), {1: ["f.txt"]})


class InTheLoop(unittest.TestCase):
    def test_handed_back_without_a_take_and_the_rest_merge(self):
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, [[fakes.pr(1), fakes.pr(2)]], {2: outcomes.MERGED})
            world.stale, said = {1: ["src/a.rs"]}, []
            world.fx.say = said.append
            _, _, status = loop.run(world.fx, fakes.opts(idle=1, poll=30))
            memory = handbacks.recall(world.fx.memory)
        self.assertEqual(world.taken, [2])
        self.assertEqual(status, ending.HANDED_BACK_STATUS)
        self.assertEqual(memory, {1: "sha1"})  # the head that was checked
        self.assertEqual(sum(s.startswith(f"{ending.HANDED_BACK_LINE} #1") for s in said), 1)
        self.assertIn("src/a.rs", next(s for s in said if s.startswith(ending.HANDED_BACK_LINE)))

    def test_a_dry_run_previews_it_and_remembers_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, [[fakes.pr(1)]], {})
            world.fx.ends = world.fx.ends | {outcomes.HANDED_BACK}
            world.stale = {1: ["a"]}
            results, _, status = loop.run(world.fx, fakes.opts("--dry-run"))
            self.assertEqual(handbacks.recall(world.fx.memory), {})
        self.assertEqual((results[0][:2], status), ((1, outcomes.HANDED_BACK), 0))


if __name__ == "__main__":
    unittest.main()
