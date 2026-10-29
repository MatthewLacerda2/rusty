#!/usr/bin/env python3
"""#1568: the queue's push is a compare-and-swap on the head it took.

A plain push accepts a branch reset backwards or deleted during the take, since
the merge commit fast-forwards from any ancestor, and would squash a commit its
author dropped (a leaked secret) into `main` (#1577's independent review). So
[`update.advance`] pushes `--force-with-lease=refs/heads/<branch>:<head>`; these
drive it over real repositories ([`test_update_by_merge.Origin`]). Also here:
the squash body leaves the merges of `main` out, and a merge that stops without
naming a path says what git said.
"""

from __future__ import annotations

import os
import tempfile
import unittest
from unittest import mock

from merging.queue import gitio, refusal, taking, update
from tests.queue.fixtures import SHA, done
from tests.queue.test_markdown_advance import WHO
from tests.queue.test_update_by_merge import Origin


class Lease(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        env = mock.patch.dict(os.environ, WHO)
        env.start()
        self.addCleanup(env.stop)
        self.repo = Origin(self.tmp.name)
        self.repo.commit({"src/lib.rs": "fn b() {}\n"}, "main")  # so the take has a merge to push

    def advance(self):
        return update.advance("b", self.repo.head, self.repo.root, push=True)

    def test_a_branch_reset_to_an_ancestor_during_the_take_is_refused_as_moved(self):
        parent = self.repo.git("rev-parse", f"{self.repo.head}~1").strip()
        self.repo.git("push", "--quiet", "--force", "origin", f"{parent}:refs/heads/b")  # the author drops a commit
        fresh, notes = self.advance()
        self.assertIsNone(fresh)
        self.assertEqual(notes[0], refusal.MOVED)
        self.assertEqual(self.repo.on_origin("b"), parent)  # the dropped commit stays dropped

    def test_a_deleted_branch_is_refused_and_not_recreated(self):
        self.repo.git("push", "--quiet", "origin", ":refs/heads/b")
        fresh, notes = self.advance()
        self.assertIsNone(fresh)
        self.assertNotEqual(notes[0], refusal.MOVED)  # no head to retake: a hand-back
        self.assertEqual(self.repo.git("ls-remote", "origin", "refs/heads/b"), "")

    def test_the_squash_body_names_the_branch_commits_and_no_merge(self):
        fresh, _ = self.advance()
        self.assertEqual(taking.squash_message(fresh, self.repo.root), "* on b")


class MergeStopped(unittest.TestCase):
    def test_a_merge_that_names_no_path_says_what_git_said(self):
        def git(*args, cwd=None):
            if "merge" in args and "origin/main" in args:
                self.assertEqual(args[:3], ("merge", "--ff", "--no-edit"))
                return done(128, "fatal: refusing to merge unrelated histories")
            return done(0)

        with mock.patch.object(gitio, "git", side_effect=git), \
                mock.patch.object(update, "main_moved_by_docs", return_value=False):
            fresh, notes = update.advance("b", SHA, ".", push=True)
        self.assertIsNone(fresh)
        self.assertIn("refusing to merge unrelated histories", notes[1])


if __name__ == "__main__":
    unittest.main()
