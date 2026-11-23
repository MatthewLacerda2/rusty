"""#2629: `make queue-quiet` starts the shepherd on a ready pull request and tells its non-`queue` verdicts.

Must start one for a ready pull request with no review decision, and for one past its verdict's head
(a round came back); must start none for a queued, held, owing, decided, non-code or draft one, nor
with `--no-shepherd` (the ready line is said instead); a start the cap refuses is asked again on the
drain pass, a failed one is said. Must say a `round` (and relaunch its coder on its `Round brief:`
issue) and a `hold` with its `Sent up:` lines, never a `queue`; each record once, across a restart,
and none of those the first poll ever reads.
"""

from __future__ import annotations

import argparse
import tempfile
import unittest
from pathlib import Path

from merging.view import quiet, shepherding
from tests.view.test_quiet import RECORD, Effects, ready

URL = "https://github.com/o/r/pull/7#issuecomment-{}"


def record(ident: int, word: str, extra: str = "", who: str = "OWNER") -> dict:
    return {"id": ident, "author_association": who, "html_url": URL.format(ident),
            "body": f"Independent review\n\n- a finding\n{extra}\nVerdict: {word} on abc1234"}


class Flock:
    def __init__(self, status=0, records=None):
        self.status, self.records, self.started, self.coders = status, records, [], []

    def fx(self, effects: Effects) -> argparse.Namespace:
        return argparse.Namespace(shepherd=lambda n: self.started.append(n) or (self.status, "why"),
                                  coder=lambda n: self.coders.append(n) or [], records=lambda: self.records,
                                  ready=lambda n: (effects.pull, effects.notes))


class Case(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / quiet.STATE

    def tearDown(self):
        self.dir.cleanup()

    def quiet(self, effects: Effects, flock: Flock | None) -> quiet.Quiet:
        fx = effects.fx()
        fx.shepherd = flock.fx(effects) if flock else None
        return quiet.Quiet(fx)


class Starts(Case):
    def test_ready_with_no_decision_or_a_round_back_starts_one_silently(self):
        for pull in (ready(), ready(verdict="round", current=False),
                     ready(verdict="round", said="queue", current=False)):
            with self.subTest(pull=pull):
                effects, flock = Effects(self.path, pull, [] if pull["standing"]["said"] is None else [RECORD]), Flock()
                self.quiet(effects, flock).line("queue: watch: #7 turned ready.")
                self.assertEqual((flock.started, effects.said), ([7], []))

    def test_decided_queued_held_owing_inert_or_draft_starts_none(self):
        for pull, notes in ((ready(), [RECORD]), (ready(labels=("queue",)), []), (ready(labels=("hold",)), []),
                            (ready(verdict="hold", current=False), [RECORD]),
                            (ready(verdict="round", current=False, owed=("a look",)), [RECORD]),
                            (ready(files=("docs/x.md",)), []), ({**ready(), "isDraft": True}, []), (None, None)):
            with self.subTest(pull=pull):
                flock = Flock()
                self.quiet(Effects(self.path, pull, notes), flock).line("queue: watch: #7 turned ready.")
                self.assertEqual(flock.started, [])

    def test_no_shepherd_says_the_ready_line_instead(self):
        effects = Effects(self.path, ready(), [])
        self.quiet(effects, None).line("queue: watch: #7 turned ready.")
        self.assertIn("no review decision", effects.said[0])

    def test_the_cap_waits_for_the_drain_pass_across_a_restart_and_a_failure_is_said(self):
        effects, flock = Effects(self.path, ready(), []), Flock(status=shepherding.AT_CAP)
        self.quiet(effects, flock).line("queue: watch: #7 turned ready.")
        flock.status = 0
        self.quiet(effects, flock).drain()
        self.assertEqual(flock.started, [7, 7])
        self.quiet(effects, flock).drain()
        self.assertEqual(flock.started, [7, 7])
        flock.status = 2
        self.quiet(effects, flock).line("queue: watch: #7 turned ready.")
        self.assertIn("could not start (exit 2)", effects.said[-1])


class Records(Case):
    def test_round_and_hold_are_said_once_queue_never_and_the_first_poll_only_marks(self):
        brief = "Round brief: https://github.com/o/r/issues/9#issuecomment-501"
        flock = Flock(records=[record(1, "round", brief)])
        effects = Effects(self.path)
        self.quiet(effects, flock).poll()
        self.assertEqual((effects.said, flock.coders), ([], []))  # posted before the watch knew of it
        flock.records += [record(2, "round", brief), record(3, "hold", "Sent up: is the cap a decision?"),
                          record(4, "queue"), record(5, "round", brief, who="NONE"),
                          {**record(6, "round", brief), "html_url": "https://github.com/o/r/issues/9#issuecomment-6"}]
        q = self.quiet(effects, flock)
        q.poll()
        q.poll()
        self.quiet(effects, flock).poll()  # a restart reads the same comments
        self.assertEqual(flock.coders, [9])
        self.assertEqual(len(effects.said), 2)
        self.assertIn("#7: round sent on abc1234, its coder relaunched on #9", effects.said[0])
        self.assertIn("#7: held on abc1234: is the cap a decision?", effects.said[1])

    def test_a_round_without_its_brief_line_asks_for_the_relaunch_by_hand(self):
        line, issue = shepherding.told(record(1, "round"))
        self.assertIsNone(issue)
        self.assertIn("relaunch it by hand", line)


if __name__ == "__main__":
    unittest.main()
