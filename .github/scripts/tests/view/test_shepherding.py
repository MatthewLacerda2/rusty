"""#2629: `make queue-quiet` starts the shepherd on a ready pull request and tells its non-`queue` verdicts.

Must start one for a ready pull request with no review decision, and for one past its verdict's head
(a round came back); must start none for a queued, held, owing, decided, non-code or draft one, nor
with `--no-shepherd` (the ready line is said instead); a start the cap refuses is asked again on the
drain pass, a failed one is said; a Dependabot bump opened ready starts none. What a record owes is
`test_shepherd_records.py`'s.
"""

from __future__ import annotations

import argparse
import tempfile
import unittest
from pathlib import Path

from merging.view import quiet, shepherding
from tests.view.test_quiet import RECORD, Effects, ready

class Flock:
    def __init__(self, status=0):
        self.status, self.started = status, []

    def fx(self, effects: Effects) -> argparse.Namespace:
        return argparse.Namespace(shepherd=lambda n: self.started.append(n) or (self.status, "why"),
                                  pulls=lambda: [], ready=lambda n: (effects.pull, effects.notes))


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
                self.path.unlink(missing_ok=True)  # each case its own watch: a start is kept by head
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

    def test_a_dependabot_bump_opened_ready_starts_none(self):
        bump = {**ready(files=("Cargo.lock",)), "headRefName": "dependabot/cargo/rustls-0.24",
                "author": {"login": "dependabot[bot]"}}
        flock = Flock()
        self.quiet(Effects(self.path, bump, []), flock).line("queue: watch: #7 opened as ready.")
        self.assertEqual(flock.started, [])
        human = {**bump, "author": {"login": "someone"}}
        self.quiet(Effects(self.path, human, []), flock).line("queue: watch: #7 opened as ready.")
        self.assertEqual(flock.started, [7], "the branch name alone exempts nobody")

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
        flock.status, effects.pull = 2, {**ready(), "headRefOid": "e" * 40}  # a new head: started once per head
        self.quiet(effects, flock).line("queue: watch: #7 turned ready.")
        self.assertIn("could not start (exit 2)", effects.said[-1])


if __name__ == "__main__":
    unittest.main()
