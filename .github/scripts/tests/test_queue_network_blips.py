#!/usr/bin/env python3
"""#847: a transient `gh` failure is asked again, never the end of a watch.

One TLS handshake timeout inside `mergeable.gh` used to `sys.exit` the whole
overnight watch with status 2. Now a network-shaped failure is retried, and
only a GitHub that stays down through every retry stops the queue, as the
machine's failure (status 3), never as a stray exit.
"""

from __future__ import annotations

import importlib.util
import subprocess
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
_spec = importlib.util.spec_from_file_location("merge_queue_blips", HERE / "merge-queue.py")
queue = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(queue)
mergeable = queue.mergeable

TLS = 'Post "https://api.github.com/graphql": net/http: TLS handshake timeout'


def answers(*replies: tuple[int, str]):
    """A `subprocess.run` stand-in replying in turn, and the calls it saw."""
    calls, left = [], list(replies)

    def run(cmd, **_):
        calls.append(cmd)
        code, text = left.pop(0)
        return subprocess.CompletedProcess(cmd, code, stdout=text if code == 0 else "", stderr="" if code == 0 else text)

    return run, calls


class Transient(unittest.TestCase):
    def test_network_failures_are_transient(self):
        for stderr in (TLS, "read tcp: i/o timeout", "connection reset by peer", "HTTP 502: Bad Gateway"):
            self.assertTrue(mergeable.transient(stderr), stderr)

    def test_request_failures_are_not(self):
        for stderr in ("HTTP 404: Not Found", "Unknown JSON field: \"nope\"", "authentication required"):
            self.assertFalse(mergeable.transient(stderr), stderr)


class Retries(unittest.TestCase):
    def test_a_blip_is_asked_again_and_answered(self):
        run, calls = answers((1, TLS), (0, '{"ok": true}'))
        slept = []
        self.assertEqual(mergeable.gh("pr", "view", "1", runner=run, sleep=slept.append), {"ok": True})
        self.assertEqual(len(calls), 2)
        self.assertEqual(slept, [mergeable.RETRIES[0]])

    def test_github_down_through_every_retry_is_unreachable(self):
        run, calls = answers(*[(1, TLS)] * (len(mergeable.RETRIES) + 1))
        slept = []
        with self.assertRaises(mergeable.Unreachable):
            mergeable.gh("pr", "view", "1", runner=run, sleep=slept.append)
        self.assertEqual(len(calls), len(mergeable.RETRIES) + 1)
        self.assertEqual(slept, list(mergeable.RETRIES))

    def test_a_real_error_is_fatal_at_once(self):
        run, calls = answers((1, "HTTP 404: Not Found"))
        with self.assertRaises(SystemExit):
            mergeable.gh("pr", "view", "1", runner=run, sleep=lambda _: self.fail("slept"))
        self.assertEqual(len(calls), 1)


class QueueStops(unittest.TestCase):
    def test_unreachable_stops_the_queue_and_names_the_rest(self):
        def turn(number):
            raise mergeable.Unreachable("gh pr view 7: " + TLS)

        results = queue.drain([7, 8], turn)
        self.assertEqual([state for _, state, _ in results], [queue.STOPPED, queue.NOT_TAKEN])
        self.assertIn("GitHub unreachable", results[0][2])

    def test_the_watch_reads_that_stop_as_the_machines(self):
        self.assertEqual(queue.queue_watch.status([], machine=True), queue.queue_watch.MACHINE_STATUS)


if __name__ == "__main__":
    unittest.main()
