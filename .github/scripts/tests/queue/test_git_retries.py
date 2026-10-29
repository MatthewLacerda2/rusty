#!/usr/bin/env python3
"""#875: a transient `git` failure is asked again, never a hand-back.

One SSL timeout on the queue's push handed back a healthy branch, and the
watch then remembered it as handed back until its head moved. The queue's own
`git` calls to GitHub now retry on `github.gh`'s backoff, every retry is the
same plain push (never forced, #1568), and a network that stays down stops the
queue as the machine's failure.
"""

from __future__ import annotations

import time
import unittest
from unittest import mock

from merging.queue import gitio, outcomes, refusal, stopping
from merging.verdict import github
from tests.queue.fixtures import FRESH, NON_FF, advancing, done


SSL = "fatal: unable to access 'https://github.com/o/r/': SSL connection timeout"
PUSH = ("push", "origin", "HEAD:refs/heads/b")


class Wording(unittest.TestCase):
    def test_git_network_failures_are_transient(self):
        for stderr in (
            SSL,
            "fatal: unable to access 'https://github.com/o/r/': Could not resolve host: github.com",
            "fatal: unable to access '…': Failed to connect to github.com port 443: Connection timed out",
            "error: RPC failed; curl 56 Recv failure: Connection reset by peer\nfatal: early EOF",
        ):
            self.assertTrue(gitio.git_transient(stderr), stderr)

    def test_refusals_are_not(self):
        for stderr in (NON_FF, "fatal: couldn't find remote ref nope", "fatal: not a git repository"):
            self.assertFalse(gitio.git_transient(stderr), stderr)


class Remote(unittest.TestCase):
    def remote(self, *replies):
        with mock.patch.object(gitio, "git", side_effect=list(replies)) as called, \
                mock.patch.object(time, "sleep") as slept:
            result = gitio.remote("fetch", "origin", "main", cwd=".")
        return result, called, [c.args[0] for c in slept.call_args_list]

    def test_a_blip_is_asked_again_with_the_same_arguments(self):
        result, called, slept = self.remote(done(128, SSL), done(0))
        self.assertEqual(result.returncode, 0)
        self.assertEqual(slept, [github.RETRIES[0]])
        self.assertEqual({c.args for c in called.call_args_list}, {("fetch", "origin", "main")})

    def test_a_refusal_is_returned_at_once(self):
        result, called, slept = self.remote(done(1, NON_FF))
        self.assertEqual((result.returncode, called.call_count, slept), (1, 1, []))

    def test_down_through_every_retry_is_unreachable(self):
        with self.assertRaises(github.Unreachable):
            self.remote(*[done(128, SSL)] * (len(github.RETRIES) + 1))


class Pushing(unittest.TestCase):
    """[`update.advance`]'s push through a flaky network."""

    def advance(self, pushes, remote_head=FRESH):
        self.calls = []
        return advancing(pushes, remote_head, self.calls)

    def pushes(self):
        return [c for c in self.calls if c[0] == "push"]

    def test_a_push_through_blips_lands_never_forced(self):
        fresh, notes = self.advance([done(128, SSL), done(128, SSL), done(0)])
        self.assertEqual((fresh, notes), (FRESH, []))
        self.assertEqual(self.pushes(), [PUSH] * 3)

    def test_a_push_that_landed_before_its_reply_was_lost_is_pushed(self):
        fresh, notes = self.advance([done(128, SSL), done(1, NON_FF)])
        self.assertEqual((fresh, notes), (FRESH, []))

    def test_a_branch_moved_since_is_refused_and_named_moved(self):
        fresh, notes = self.advance([done(1, NON_FF)], remote_head="2" * 40)
        self.assertIsNone(fresh)
        self.assertEqual(notes[0], refusal.MOVED)

    def test_a_network_down_through_every_retry_stops_and_cleans_up(self):
        with self.assertRaises(github.Unreachable):
            self.advance([done(128, SSL)] * (len(github.RETRIES) + 1))
        self.assertEqual(self.calls[-1][:2], ("worktree", "remove"))


class Stopping(unittest.TestCase):
    def test_retries_exhausted_mid_take_stop_the_queue_not_hand_back(self):
        def turn(number):
            raise github.Unreachable(f"git push: {SSL}")

        with mock.patch("builtins.print"):
            results = stopping.drain([846, 865], turn)
        self.assertEqual([state for _, state, _ in results], [outcomes.STOPPED, outcomes.NOT_TAKEN])
        self.assertIn("SSL connection timeout", results[0][2])


if __name__ == "__main__":
    unittest.main()
