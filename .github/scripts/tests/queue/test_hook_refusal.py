#!/usr/bin/env python3
"""#1108: a refused push is handed back with what refused it.

Since #1101 `.githooks/pre-push` runs on the queue's own force-pushes, so a
refusal is GitHub, the hook or the lease ([`refusal`]). The stderr decides
first; only a refusal it does not name falls back to the remote head.
"""

from __future__ import annotations

import unittest

from tests.queue.fixtures import SHA, STALE, advancing, done


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
        self.assertEqual(notes[0], f"the pre-push hook refused the force-push of b: {FMT}")
        self.assertTrue(any("1111111" in n and "make resolved" in n for n in notes))
        self.assertIn("+mod a;", notes)
        self.assertNotIn("something else has pushed", " ".join(notes))

    def test_the_hook_still_when_a_push_landed_since(self):
        self.assertIn("pre-push hook refused", refused(HOOK, OTHER)[0])

    def test_a_missing_cargo_is_the_hook(self):
        notes = refused(NO_CARGO, OTHER)
        self.assertEqual(
            notes[0],
            "the pre-push hook refused the force-push of b:"
            " FileNotFoundError: [Errno 2] No such file or directory: 'cargo'",
        )
        self.assertIn("`cargo` on its PATH", notes[1])

    def test_github_rejecting_it_is_never_the_hook(self):
        notes = refused(WORKFLOW, SHA)
        self.assertIn("GitHub rejected the force-push of b", notes[0])
        self.assertIn("`workflow` scope", notes[0])
        self.assertNotIn("hook", " ".join(notes))

    def test_a_moved_head_is_the_lease(self):
        notes = refused(STALE, OTHER)
        self.assertIn("the force-push of b was refused: ! [rejected]", notes[0])
        self.assertIn("something else has pushed since", notes[1])
        self.assertNotIn("hook", " ".join(notes))

    def test_an_unreadable_head_is_the_lease(self):
        self.assertIn("something else has pushed since", refused(STALE, None)[1])

    def test_the_reason_is_one_line(self):
        for stderr in (HOOK, NO_CARGO, WORKFLOW, STALE):
            self.assertNotIn("\n", refused(stderr, SHA)[0], stderr)


if __name__ == "__main__":
    unittest.main()
