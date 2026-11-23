"""#2629: the watch finds what to start, and remembers what it did, from GitHub and `quiet.json` alone.

Must start a shepherd on a pull request already ready when the watch starts, with no log line; must
start it on the next poll when the compare read failed at the flip; must start none a second time on
one head (two polls, a "turned ready" line, a restart); with `--no-shepherd` must say its ready line
once per head instead. Must relaunch a round's coder once even after more than 20 newer routines push
that coder off the routines listing.
"""

from __future__ import annotations

import argparse
import tempfile
import unittest
from pathlib import Path

from merging.view import quiet
from tests.view.test_quiet import Effects, ready
from tests.view.test_shepherd_records import BRIEF, GitHub, pull, record

HEAD = "f" * 40


def listed(stamp: str = "u1", head: str = HEAD) -> dict:
    return {"number": 7, "draft": False, "head": {"sha": head}, "updated_at": stamp}


class Ready:
    """One ready pull request on GitHub, its standing read failing while `unread` is set."""

    def __init__(self, on: bool = True):
        self.pulls, self.pull, self.unread, self.on = [listed()], ready(), False, on
        self.started, self.reads = [], 0

    def read(self, number: int):
        self.reads += 1
        standing = {**self.pull["standing"], "unread": self.unread}
        return {**self.pull, "standing": standing}, []

    def fx(self) -> argparse.Namespace:
        start = (lambda n: self.started.append(n) or (0, "")) if self.on else None
        return argparse.Namespace(shepherd=start, ready=self.read, pulls=lambda: self.pulls,
                                  thread=lambda n: [], coders=lambda: [], waiting=set, coder=lambda i: [])


class Case(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.effects = Effects(Path(self.dir.name) / quiet.STATE)

    def tearDown(self):
        self.dir.cleanup()

    def stream(self, flock) -> quiet.Quiet:
        fx = self.effects.fx()
        fx.shepherd = flock.fx()
        return quiet.Quiet(fx)


class Starts(Case):
    def test_a_pull_request_ready_before_the_watch_started_gets_one_on_its_first_poll(self):
        github = Ready()
        self.stream(github).poll()
        self.assertEqual((github.started, self.effects.said), ([7], []))

    def test_a_compare_unread_at_the_flip_is_started_on_the_next_poll(self):
        github = Ready()
        github.unread = True
        stream = self.stream(github)
        stream.line("queue: watch: #7 turned ready.")
        stream.poll()
        self.assertEqual(github.started, [])
        github.unread = False
        stream.poll()
        self.assertEqual(github.started, [7])

    def test_no_second_start_on_one_head_across_polls_a_ready_line_and_a_restart(self):
        github = Ready()
        stream = self.stream(github)
        stream.poll()
        github.pulls = [listed(stamp="u2")]  # a comment moved it, the head did not
        stream.poll()
        stream.line("queue: watch: #7 is ready and unqueued.")
        self.stream(github).poll()
        self.assertEqual(github.started, [7])
        github.pulls, github.pull = [listed("u3", "e" * 40)], {**ready(), "headRefOid": "e" * 40}
        self.stream(github).poll()
        self.assertEqual(github.started, [7, 7], "a new head of its own is a new start")

    def test_one_that_owes_nothing_is_read_once_until_it_moves(self):
        github = Ready()
        github.pull = ready(labels=("hold",))
        stream = self.stream(github)
        stream.poll()
        stream.poll()
        self.assertEqual((github.started, github.reads), ([], 1))

    def test_no_shepherd_says_the_ready_line_once_per_head(self):
        github = Ready(on=False)
        self.stream(github).poll()
        self.stream(github).poll()
        self.assertEqual(len(self.effects.said), 1, self.effects.said)
        self.assertIn("#7 is ready with no review decision", self.effects.said[0])


class Relaunches(Case):
    def test_a_relaunch_is_kept_past_twenty_newer_routines(self):
        github = GitHub([pull()], {7: [record(1, "round", f"Round brief: {BRIEF}")]})
        self.stream(github).poll()
        self.assertEqual(github.launched, [9])
        github.routines = [{"name": f"rusty #{100 + n} x", "created_at": "2026-11-23T11:00:00Z"} for n in range(21)]
        github.pulls = [pull(stamp="u2")]
        self.stream(github).poll()
        self.assertEqual(github.launched, [9], "the coder's routine is off the listing; quiet.json remembers")


if __name__ == "__main__":
    unittest.main()
