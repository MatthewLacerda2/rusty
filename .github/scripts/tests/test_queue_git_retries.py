#!/usr/bin/env python3
"""#875: a transient `git` failure is asked again, never a hand-back.

One SSL timeout on the queue's force-push handed back a healthy branch, and the
watch then remembered it as handed back until its head moved. The queue's own
`git` calls to GitHub now retry on `mergeable.gh`'s backoff, the push keeps its
lease on the head the take began from, and a network that stays down stops the
queue as the machine's failure.
"""

from __future__ import annotations

import importlib.util
import subprocess
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent.parent
_spec = importlib.util.spec_from_file_location("merge_queue_git_retries", HERE / "merge-queue.py")
queue = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(queue)
mergeable = queue.mergeable

SHA, FRESH = "a" * 40, "1" * 40
SSL = "fatal: unable to access 'https://github.com/o/r/': SSL connection timeout"
STALE = " ! [rejected]        b -> b (stale info)\nerror: failed to push some refs"
LEASE = f"--force-with-lease=refs/heads/b:{SHA}"


def done(code: int, stderr: str = "", stdout: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess([], code, stdout, stderr)


class Wording(unittest.TestCase):
    def test_git_network_failures_are_transient(self):
        for stderr in (
            SSL,
            "fatal: unable to access 'https://github.com/o/r/': Could not resolve host: github.com",
            "fatal: unable to access '…': Failed to connect to github.com port 443: Connection timed out",
            "error: RPC failed; curl 56 Recv failure: Connection reset by peer\nfatal: early EOF",
        ):
            self.assertTrue(queue.git_transient(stderr), stderr)

    def test_refusals_are_not(self):
        for stderr in (STALE, "fatal: couldn't find remote ref nope", "fatal: not a git repository"):
            self.assertFalse(queue.git_transient(stderr), stderr)


class Remote(unittest.TestCase):
    def remote(self, *replies):
        with mock.patch.object(queue, "git", side_effect=list(replies)) as called, \
                mock.patch.object(queue.time, "sleep") as slept:
            result = queue.remote("fetch", "origin", "main", cwd=".")
        return result, called, [c.args[0] for c in slept.call_args_list]

    def test_a_blip_is_asked_again_with_the_same_arguments(self):
        result, called, slept = self.remote(done(128, SSL), done(0))
        self.assertEqual(result.returncode, 0)
        self.assertEqual(slept, [mergeable.RETRIES[0]])
        self.assertEqual({c.args for c in called.call_args_list}, {("fetch", "origin", "main")})

    def test_a_refusal_is_returned_at_once(self):
        result, called, slept = self.remote(done(1, STALE))
        self.assertEqual((result.returncode, called.call_count, slept), (1, 1, []))

    def test_down_through_every_retry_is_unreachable(self):
        with self.assertRaises(mergeable.Unreachable):
            self.remote(*[done(128, SSL)] * (len(mergeable.RETRIES) + 1))


class Pushing(unittest.TestCase):
    """[`queue.advance`]'s push through a flaky network."""

    def advance(self, pushes, remote_head=FRESH):
        calls, pushes = [], list(pushes)

        def git(*args, cwd=None):
            calls.append(args)
            if args[0] == "rev-parse":
                return done(0, stdout=FRESH + "\n")
            if args[0] == "push":
                return pushes.pop(0)
            if args[0] == "ls-remote":
                return done(0, stdout=f"{remote_head}\trefs/heads/b\n" if remote_head else "")
            return done(0)

        with mock.patch.object(queue, "git", side_effect=git), \
                mock.patch.object(queue, "main_moved_by_docs", return_value=False), \
                mock.patch.object(queue.time, "sleep"):
            try:
                return queue.advance("b", SHA, ".", push=True)
            finally:
                self.calls = calls

    def pushes(self):
        return [c for c in self.calls if c[0] == "push"]

    def test_a_push_through_blips_lands_and_keeps_its_lease(self):
        fresh, notes = self.advance([done(128, SSL), done(128, SSL), done(0)])
        self.assertEqual((fresh, notes), (FRESH, []))
        self.assertEqual(len(self.pushes()), 3)
        self.assertTrue(all(LEASE in p for p in self.pushes()))

    def test_a_push_that_landed_before_its_reply_was_lost_is_pushed(self):
        fresh, notes = self.advance([done(128, SSL), done(1, STALE)])
        self.assertEqual((fresh, notes), (FRESH, []))

    def test_a_real_refusal_is_still_handed_back(self):
        fresh, notes = self.advance([done(1, STALE)], remote_head="2" * 40)
        self.assertIsNone(fresh)
        self.assertIn("refused", notes[0])

    def test_a_network_down_through_every_retry_stops_and_cleans_up(self):
        with self.assertRaises(mergeable.Unreachable):
            self.advance([done(128, SSL)] * (len(mergeable.RETRIES) + 1))
        self.assertEqual(self.calls[-1][:2], ("worktree", "remove"))


class Stopping(unittest.TestCase):
    def test_retries_exhausted_mid_take_stop_the_queue_not_hand_back(self):
        def turn(number):
            raise mergeable.Unreachable(f"git push: {SSL}")

        with mock.patch("builtins.print"):
            results = queue.drain([846, 865], turn)
        self.assertEqual([state for _, state, _ in results], [queue.STOPPED, queue.NOT_TAKEN])
        self.assertIn("SSL connection timeout", results[0][2])


if __name__ == "__main__":
    unittest.main()
