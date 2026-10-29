#!/usr/bin/env python3
"""#1568: the queue merges `main` into a branch and pushes it without force.

These drive [`update.advance`] over real repositories, a bare `origin` and a
clone of it, so every push is a real one: a branch already holding `main` is
left alone, a code advance is a merge commit pushed as a fast-forward, a
conflict is handed back with its paths and nothing pushed, and a branch pushed
to during the take is refused by GitHub's fast-forward rule (here, git's) and
never overwritten, which [`taking.take`] reads as "not ready", not a hand-back.
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from merging.queue import gitio, outcomes, refusal, taking, update
from tests.queue.test_markdown_advance import WHO
from tests.queue import test_taking


class Origin:
    """A bare `origin` with `main` and a branch `b` one commit off its base, and
    a clone of it (`root`) that the queue runs in."""

    def __init__(self, tmp: str):
        self.bare, self.root = os.path.join(tmp, "origin.git"), os.path.join(tmp, "root")
        run("git", "init", "--quiet", "--bare", "--initial-branch=main", self.bare)
        run("git", "clone", "--quiet", self.bare, self.root)
        hooks = os.path.join(tmp, "no-hooks")
        os.mkdir(hooks)
        self.git("config", "core.hooksPath", hooks)  # the repo's pre-push hook is not under test
        self.git("checkout", "--quiet", "-b", "main")
        self.commit({"src/lib.rs": "fn a() {}\n"}, "main")
        self.git("checkout", "--quiet", "-b", "b")
        self.head = self.commit({"src/feature.rs": "fn f() {}\n"}, "b")
        self.git("checkout", "--quiet", "main")

    def git(self, *args: str) -> str:
        return run("git", *args, cwd=self.root)

    def commit(self, files: dict[str, str], branch: str) -> str:
        for path, text in files.items():
            (Path(self.root) / path).parent.mkdir(parents=True, exist_ok=True)
            (Path(self.root) / path).write_text(text)
        self.git("add", "-A")
        self.git("commit", "--quiet", "-m", f"on {branch}")
        self.git("push", "--quiet", "origin", f"HEAD:refs/heads/{branch}")
        self.git("fetch", "--quiet", "origin")
        return self.git("rev-parse", "HEAD").strip()

    def on_origin(self, branch: str) -> str:
        return run("git", "rev-parse", f"refs/heads/{branch}", cwd=self.bare).strip()


def run(*argv: str, cwd: str | None = None) -> str:
    return subprocess.run(argv, cwd=cwd, capture_output=True, text=True, check=True).stdout


class Updating(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        env = mock.patch.dict(os.environ, WHO)
        env.start()
        self.addCleanup(env.stop)
        self.repo = Origin(self.tmp.name)

    def advance(self):
        calls = []
        real = gitio.git

        def spy(*args, cwd=None):
            calls.append(args)
            return real(*args, cwd=cwd)

        with mock.patch.object(gitio, "git", side_effect=spy):
            fresh, notes = update.advance("b", self.repo.head, self.repo.root, push=True)
        self.pushes = [c for c in calls if c[0] == "push"]
        self.assertNotIn("rebase", [c[0] for c in calls])
        self.assertFalse(any("force" in a for push in self.pushes for a in push), self.pushes)
        return fresh, notes

    def test_a_branch_already_holding_main_is_a_no_op(self):
        self.assertEqual(self.advance(), (self.repo.head, []))
        self.assertEqual(self.pushes, [])
        self.assertEqual(self.repo.on_origin("b"), self.repo.head)

    def test_a_code_advance_is_a_merge_pushed_without_force(self):
        main = self.repo.commit({"src/lib.rs": "fn b() {}\n"}, "main")
        fresh, notes = self.advance()
        self.assertEqual(notes, [])
        self.assertEqual(self.repo.on_origin("b"), fresh)
        parents = self.repo.git("rev-list", "--parents", "-n", "1", fresh).split()[1:]
        self.assertEqual(parents, [self.repo.head, main])  # the branch's commit kept, not rewritten
        self.assertEqual(self.pushes, [("push", "origin", "HEAD:refs/heads/b")])

    def test_a_conflict_is_handed_back_naming_its_paths_and_nothing_is_pushed(self):
        self.repo.commit({"src/feature.rs": "fn g() {}\n"}, "main")
        fresh, notes = self.advance()
        self.assertIsNone(fresh)
        self.assertIn("Conflicted: src/feature.rs.", notes)
        self.assertEqual(self.pushes, [])
        self.assertEqual(self.repo.on_origin("b"), self.repo.head)
        self.assertNotIn("rusty-queue-", self.repo.git("worktree", "list"))

    def test_a_branch_moved_during_the_take_is_refused_not_overwritten(self):
        self.repo.commit({"src/lib.rs": "fn b() {}\n"}, "main")
        self.repo.git("checkout", "--quiet", "--detach", self.repo.head)
        foreign = self.repo.commit({"src/other.rs": "fn o() {}\n"}, "b")  # someone else's push
        fresh, notes = self.advance()
        self.assertIsNone(fresh)
        self.assertEqual(notes[0], refusal.MOVED)
        self.assertEqual(self.repo.on_origin("b"), foreign)
        self.assertEqual(len(self.pushes), 1)


class Moved(unittest.TestCase):
    """[`taking.take`] of a branch moved mid-take: not ready, never handed back."""

    def test_a_moved_branch_is_not_ready_and_nothing_merges(self):
        (_, state, why), _, called, _ = test_taking.Taking.take(self, advance=(None, [refusal.MOVED, "The push of b was no fast-forward."]))
        self.assertEqual((state, why), (outcomes.NOT_READY, refusal.MOVED))
        called.assert_not_called()
        self.assertEqual(taking.unready((1, state, why))[1], outcomes.NOT_READY)


if __name__ == "__main__":
    unittest.main()
