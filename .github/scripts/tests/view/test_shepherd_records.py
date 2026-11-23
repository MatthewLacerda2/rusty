"""#2629: what a `round` or `hold` record owes, worked out by `make queue-quiet` from GitHub alone.

Must relaunch the coder of a draft pull request whose newest record is a `round` on its head, however
old the record and on the first poll a stream makes (a watch started an hour after it), and only
once: two polls, or a restart, see the routine or the waiting line and launch none. Must launch none
for a `round` on an older head, a ready one, a `queue`, or one by someone who cannot push, and wait
when the routines are not listed. Must say a `hold` with its `Sent up:` lines once across a restart,
and a `round` with no `Round brief:` line asks for the relaunch by hand.
"""

from __future__ import annotations

import argparse
import tempfile
import unittest
from pathlib import Path

from merging.view import quiet
from tests.view.test_quiet import Effects

HEAD = "abc1234" + "0" * 33
BRIEF = "https://github.com/o/r/issues/9#issuecomment-501"
POSTED = "2026-11-23T09:00:00Z"


def record(ident: int, word: str, extra: str = "", who: str = "OWNER", at: str = POSTED) -> dict:
    return {"id": ident, "author_association": who, "created_at": at,
            "html_url": f"https://github.com/o/r/pull/7#issuecomment-{ident}",
            "body": f"Independent review\n\n- a finding\n{extra}\nVerdict: {word} on abc1234"}


def pull(number: int = 7, draft: bool = True, head: str = HEAD, stamp: str = "u1") -> dict:
    return {"number": number, "draft": draft, "head": {"sha": head}, "updated_at": stamp}


ROUND = record(1, "round", f"Round brief: {BRIEF}")


class GitHub:
    """The open pull requests, their threads, the routines and the waiting line; a relaunch
    creates a coder routine (or a place in the line) the way `make coder` does."""

    def __init__(self, pulls, threads, routines=(), queue_it=False):
        self.pulls, self.threads, self.routines = list(pulls), threads, list(routines)
        self.line, self.queue_it, self.launched, self.reads = set(), queue_it, [], 0

    def coder(self, issue: int) -> list[str]:
        self.launched.append(issue)
        if self.queue_it:
            self.line.add(issue)
        else:
            self.routines.append({"name": f"rusty #{issue} x", "created_at": "2026-11-23T10:00:00Z"})
        return []

    def thread(self, number: int):
        self.reads += 1
        return self.threads.get(number)

    def fx(self) -> argparse.Namespace:
        return argparse.Namespace(shepherd=lambda n: (0, ""), ready=lambda n: (None, None),
                                  pulls=lambda: self.pulls, thread=self.thread, coder=self.coder,
                                  coders=lambda: None if self.routines is None else list(self.routines),
                                  waiting=lambda: set(self.line))


class Case(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.effects = Effects(Path(self.dir.name) / quiet.STATE)

    def tearDown(self):
        self.dir.cleanup()

    def stream(self, github: GitHub) -> quiet.Quiet:
        fx = self.effects.fx()
        fx.shepherd = github.fx()
        return quiet.Quiet(fx)


class Rounds(Case):
    def test_a_watch_started_an_hour_after_a_round_relaunches_its_coder_on_its_first_poll(self):
        github = GitHub([pull()], {7: [record(0, "queue", at="2026-11-22T00:00:00Z"), ROUND]},
                        routines=[{"name": "rusty #9 x", "created_at": "2026-11-23T08:00:00Z"}])
        self.stream(github).poll()
        self.assertEqual(github.launched, [9])
        self.assertIn("#7: round sent on abc1234, its coder relaunched on #9", self.effects.said[0])

    def test_two_polls_in_a_row_or_a_restart_relaunch_once(self):
        for queue_it in (False, True):
            with self.subTest(queue_it=queue_it):
                github = GitHub([pull()], {7: [ROUND]}, queue_it=queue_it)
                stream = self.stream(github)
                stream.poll()
                github.pulls = [pull(stamp="u2")]  # a comment moved `updated_at`: the thread is read again
                stream.poll()
                self.stream(github).poll()
                self.assertEqual(github.launched, [9])

    def test_a_coder_on_its_brief_already_counts_however_it_was_launched(self):
        made = {"name": "rusty #9 x", "created_at": "2026-11-23T08:59:00Z", "job_config": {"prompt": BRIEF}}
        github = GitHub([pull()], {7: [ROUND]}, routines=[made])
        self.stream(github).poll()
        self.assertEqual(github.launched, [])

    def test_no_relaunch_off_its_head_ready_queued_untrusted_or_unlisted(self):
        for pulls, threads, routines in (
                ([pull(head="f" * 40)], {7: [ROUND]}, []), ([pull(draft=False)], {7: [ROUND]}, []),
                ([pull()], {7: [record(1, "queue")]}, []), ([pull()], {7: [{**ROUND, "author_association": "NONE"}]}, []),
                ([pull()], {7: [ROUND]}, None), ([pull()], {}, [])):
            with self.subTest(threads=threads, routines=routines):
                github = GitHub(pulls, threads, routines or [])
                github.routines = routines
                self.stream(github).poll()
                self.assertEqual(github.launched, [])

    def test_a_thread_is_read_again_only_when_its_pull_request_moved(self):
        github = GitHub([pull()], {7: [record(1, "queue")]})
        stream = self.stream(github)
        stream.poll()
        stream.poll()
        self.assertEqual(github.reads, 1)


class Holds(Case):
    def test_a_hold_is_said_once_across_a_restart_and_a_round_without_its_brief_asks_by_hand(self):
        github = GitHub([pull(draft=False), pull(8)],
                        {7: [record(3, "hold", "Sent up: is the cap a decision?")], 8: [record(4, "round")]})
        self.stream(github).poll()
        self.stream(github).poll()
        self.assertEqual(len(self.effects.said), 2, self.effects.said)
        self.assertIn("#7: held on abc1234: is the cap a decision?", self.effects.said[0])
        self.assertIn("#8: round sent on abc1234 with no `Round brief:` line: relaunch its coder by hand",
                      self.effects.said[1])
        self.assertEqual(github.launched, [])


if __name__ == "__main__":
    unittest.main()
