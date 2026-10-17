#!/usr/bin/env python3
"""#967: the watch ends itself when its own code changes on `main`.

At launch it notes the blob of every file it runs ([`freshness.launched`]);
before each take it compares them with `origin/main` ([`freshness.changed`]).
A changed blob ends the watch with the pull request untaken; an unchanged one
does not. Driven with a real git repository and the loop tests' fake world.
"""

from __future__ import annotations

import subprocess
import tempfile
import types
import unittest
from pathlib import Path

from merging.queue import outcomes
from merging.watch import ending, freshness, loop
from tests.watch import fakes
from tests.watch.fakes import pr


def git(*args, cwd):
    done = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=False)
    return done.returncode, done.stdout


def repo(tmp: str) -> Path:
    """A checkout whose `origin/main` is its own `main`, holding the queue's code."""
    code = Path(tmp, ".github", "scripts", "merging")
    code.mkdir(parents=True)
    (code / "loop.py").write_text("old\n")
    for args in (("init", "-q", "-b", "main"), ("config", "user.email", "t@t"), ("config", "user.name", "t"),
                 ("add", "."), ("commit", "-q", "-m", "base"), ("update-ref", "refs/remotes/origin/main", "main")):
        git(*args, cwd=tmp)
    return code / "loop.py"


class Changed(unittest.TestCase):
    def test_an_unchanged_blob_is_not_a_change_and_a_changed_one_is(self):
        with tempfile.TemporaryDirectory() as tmp:
            file = repo(tmp)
            blobs = freshness.launched([file], git)
            at = lambda *a: git(*a, cwd=tmp)  # noqa: E731
            self.assertEqual(blobs.keys(), {".github/scripts/merging/loop.py"})
            self.assertEqual(freshness.changed(blobs, at), [])
            file.write_text("new\n")
            git("commit", "-q", "-am", "the queue changes", cwd=tmp)
            git("update-ref", "refs/remotes/origin/main", "main", cwd=tmp)
            self.assertEqual(freshness.changed(blobs, at), [".github/scripts/merging/loop.py"])

    def test_git_that_cannot_read_main_fails_open(self):
        self.assertEqual(freshness.changed({"a.py": "x"}, lambda *a: (128, "")), [])

    def test_the_running_files_are_the_package_and_the_entry_script(self):
        here = Path(__file__)
        modules = {
            "merging.watch.loop": types.SimpleNamespace(__file__=str(here)),
            "json": types.SimpleNamespace(__file__="/usr/lib/json.py"),
            "mergingly": types.SimpleNamespace(__file__="/x/mergingly.py"),
        }
        files = freshness.running(modules, entry="/r/merge-queue.py")
        self.assertEqual(files, sorted([here.resolve(), Path("/r/merge-queue.py").resolve()]))
        self.assertIn(Path(loop.__file__).resolve(), freshness.running())


class InTheLoop(unittest.TestCase):
    def run_world(self, moved):
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, [[pr(1), pr(2)]], {1: outcomes.MERGED, 2: outcomes.MERGED})
            world.code_moved = moved
            results, why, status = loop.run(world.fx, fakes.opts())
        return world, why, status

    def test_a_change_on_main_ends_the_watch_before_the_next_take(self):
        world, why, status = self.run_world(["merging/watch/loop.py"])
        self.assertEqual(world.taken, [])
        self.assertEqual(status, ending.RELAUNCH_STATUS)
        self.assertTrue(why.startswith(ending.RELAUNCH))
        self.assertIn("merging/watch/loop.py", why)

    def test_no_change_takes_everything(self):
        world, why, status = self.run_world([])
        self.assertEqual((world.taken, status), ([1, 2], 0))

    def test_a_relaunch_outranks_a_hand_back_but_not_the_machine(self):
        self.assertEqual(ending.status([7], False, True), ending.RELAUNCH_STATUS)
        self.assertEqual(ending.status([], True, True), ending.MACHINE_STATUS)


if __name__ == "__main__":
    unittest.main()
