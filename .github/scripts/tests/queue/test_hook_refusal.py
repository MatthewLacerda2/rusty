#!/usr/bin/env python3
"""#1108: a push the pre-push hook refused is not called a lost lease.

Since #1101 `.githooks/pre-push` runs on the queue's own force-pushes, so a
refusal is either the hook or the lease. GitHub still holding the head the take
began from says the lease held, so the hook refused it.
"""

from __future__ import annotations

import subprocess
import time
import unittest
from unittest import mock

from merging.queue import gitio, rebase


SHA, FRESH, OTHER = "a" * 40, "1" * 40, "2" * 40
FMT = "resolved: `cargo fmt --all --check` failed; run `cargo fmt --all` and commit"
NO_CARGO = "FileNotFoundError: [Errno 2] No such file or directory: 'cargo'"
STALE = " ! [rejected]        b -> b (stale info)\nerror: failed to push some refs"


def done(code: int, stderr: str = "", stdout: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess([], code, stdout, stderr)


def advance(stderr: str, remote_head: str):
    def git(*args, cwd=None):
        if args[0] == "rev-parse":
            return done(0, stdout=FRESH + "\n")
        if args[0] == "push":
            return done(1, stderr)
        if args[0] == "ls-remote":
            return done(0, stdout=f"{remote_head}\trefs/heads/b\n")
        return done(0)

    with mock.patch.object(gitio, "git", side_effect=git), \
            mock.patch.object(rebase, "main_moved_by_docs", return_value=False), \
            mock.patch.object(time, "sleep"):
        return rebase.advance("b", SHA, ".", push=True)


class Refusal(unittest.TestCase):
    def test_an_unmoved_remote_head_is_the_hook(self):
        fresh, notes = advance(f"{FMT}\nerror: failed to push some refs", remote_head=SHA)
        self.assertIsNone(fresh)
        self.assertIn("pre-push hook refused", notes[0])
        self.assertIn(FMT, notes[0])
        self.assertIn("make resolved", notes[-1])
        self.assertNotIn("something else has pushed", " ".join(notes))

    def test_a_missing_cargo_is_the_hook_too(self):
        fresh, notes = advance(f"Traceback …\n{NO_CARGO}", remote_head=SHA)
        self.assertIsNone(fresh)
        self.assertIn("pre-push hook refused", notes[0])
        self.assertTrue(any("`cargo` on its PATH" in n for n in notes))

    def test_a_moved_remote_head_is_the_lease(self):
        fresh, notes = advance(STALE, remote_head=OTHER)
        self.assertIsNone(fresh)
        self.assertIn("the force-push of b was refused", notes[0])
        self.assertIn("something else has pushed since", notes[1])
        self.assertNotIn("hook", " ".join(notes))


if __name__ == "__main__":
    unittest.main()
