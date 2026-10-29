#!/usr/bin/env python3
"""#1108: a refused push is handed back with what refused it.

Since #1101 `.githooks/pre-push` runs on the queue's own pushes, so a
refusal is GitHub, the hook or a moved branch ([`refusal`]); since #1568 the
last is no hand-back but [`refusal.MOVED`]. The stderr decides
first; only a refusal it does not name falls back to the remote head.
"""

from __future__ import annotations

import unittest

from merging.queue import refusal
from tests.queue.fixtures import SHA, NON_FF, advancing, done


OTHER = "2" * 40
FMT = "resolved: `cargo fmt --all --check` failed; run `cargo fmt --all` and commit"
HOOK = f"Diff in src/x.rs:\n-mod b;\n+mod a;\n{FMT}\nerror: failed to push some refs to 'o/r'"
NO_CARGO = (
    "Traceback (most recent call last):\n  File \"resolved.py\", line 9\n"
    "FileNotFoundError: [Errno 2] No such file or directory: 'cargo'\n"
    "error: failed to push some refs to 'o/r'"
)
WORKFLOW = (
    " ! [remote rejected] HEAD -> b (refusing to allow an OAuth App to create or"
    " update workflow `.github/workflows/ci.yml` without `workflow` scope)\n"
    "error: failed to push some refs to 'o/r'"
)


def refused(stderr: str, remote_head):
    fresh, notes = advancing([done(1, stderr)], remote_head)
    assert fresh is None
    return notes


class Refusal(unittest.TestCase):
    def test_the_hook_with_the_head_unmoved(self):
        notes = refused(HOOK, SHA)
        self.assertEqual(notes[0], f"the pre-push hook refused the push of b: {FMT}")
        self.assertTrue(any("1111111" in n and "make resolved" in n for n in notes))
        self.assertIn("+mod a;", notes)
        self.assertNotIn("something else has pushed", " ".join(notes))

    def test_the_hook_still_when_a_push_landed_since(self):
        self.assertIn("pre-push hook refused", refused(HOOK, OTHER)[0])

    def test_a_missing_cargo_is_the_hook(self):
        notes = refused(NO_CARGO, OTHER)
        self.assertEqual(
            notes[0],
            "the pre-push hook refused the push of b:"
            " FileNotFoundError: [Errno 2] No such file or directory: 'cargo'",
        )
        self.assertIn("`cargo` on its PATH", notes[1])

    def test_github_rejecting_it_is_never_the_hook(self):
        notes = refused(WORKFLOW, SHA)
        self.assertIn("GitHub rejected the push of b", notes[0])
        self.assertIn("`workflow` scope", notes[0])
        self.assertNotIn("hook", " ".join(notes))

    def test_a_moved_head_is_a_moved_branch(self):
        notes = refused(NON_FF, OTHER)
        self.assertEqual(notes[0], refusal.MOVED)
        self.assertIn("refused by its lease: ! [rejected]", notes[1])
        self.assertNotIn("hook", " ".join(notes))

    def test_an_unreadable_head_is_a_hand_back_never_moved(self):
        # An expired token reading as "moved" would have the watch retake it forever.
        notes = refused(NON_FF, None)
        self.assertNotEqual(notes[0], refusal.MOVED)
        self.assertIn("could not be read", notes[0])
        self.assertIn("Handed back.", notes[1])

    def test_the_reason_is_one_line(self):
        for stderr in (HOOK, NO_CARGO, WORKFLOW, NON_FF):
            self.assertNotIn("\n", refused(stderr, SHA)[0], stderr)


if __name__ == "__main__":
    unittest.main()
