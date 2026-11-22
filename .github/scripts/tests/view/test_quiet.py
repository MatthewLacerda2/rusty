"""#2628: `make queue-quiet` handles a batch's routine itself and says only what needs someone.

Must say: a hand-back, a stopped supervisor, a ready pull request with no review
decision (or reviewed with nothing that queues it, or owing a `Needs:`), a new
bug, a red `main` once per head, a drain pass failing three times in a row.
Must not: a merge (it fast-forwards and reclaims instead, once), a draft opening,
a close, a ready pull request already queued or under a `round`/`hold`
verdict, the bugs and red runs a restart already told.
"""

from __future__ import annotations

import argparse
import tempfile
import unittest
from pathlib import Path

from merging.view import quiet

RECORD = {"body": "Independent review\nNo findings.", "author_association": "OWNER", "created_at": "t"}


def ready(labels=(), verdict=None, owed=(), files=("src/a.rs",)) -> dict:
    return {"number": 7, "state": "OPEN", "isDraft": False, "baseRefName": "main", "headRefName": "7-x",
            "labels": [{"name": n} for n in labels], "files": [{"path": f} for f in files],
            "standing": {"verdict": verdict, "owed": list(owed)}}


class Effects:
    def __init__(self, path: Path, pull=None, comments=(), bugs=(), runs=(), drain=()):
        self.path, self.said, self.merges = path, [], []
        self.pull, self.notes, self.issues, self.runs, self.failed = pull, comments, list(bugs), list(runs), list(drain)

    def fx(self):
        return argparse.Namespace(
            after_merge=lambda n: self.merges.append(n) or [], drain=lambda: self.failed,
            ready=lambda n: (self.pull, self.notes), bugs=lambda: self.issues, main_runs=lambda: self.runs,
            comments=lambda: [], say=self.said.append, path=self.path)


class Lines(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / quiet.STATE

    def tearDown(self):
        self.dir.cleanup()

    def stream(self, effects: Effects, *lines: str) -> list[str]:
        q = quiet.Quiet(effects.fx())
        for line in lines:
            q.line(line)
        return effects.said

    def test_a_merge_is_handled_once_and_said_by_nobody(self):
        effects = Effects(self.path)
        said = self.stream(effects, "queue: #12: merged.", "queue: #12: merged — landed with the train abc.", "queue: watch: #12 closed, not merged by this watch.")
        self.assertEqual((said, effects.merges), ([], [12]))

    def test_passes_a_hand_back_and_a_stop_drops_the_board_chatter(self):
        said = self.stream(Effects(self.path), "queue: HANDED BACK #3: CI is red.", "supervisor: crashed: boom; stopped.",
                           "queue: watch: #4 opened as a draft.", "queue: watch: #4 back to draft.", "queue: watch: taking #4.")
        self.assertEqual(said, ["queue: HANDED BACK #3: CI is red.", "supervisor: crashed: boom; stopped."])

    def test_a_ready_pull_request_that_needs_someone(self):
        for pull, comments, words in (
            (ready(), [], "no review decision"),
            (ready(), [RECORD], "nothing queues it"),
            (ready(files=("docs/x.md",)), [], "nothing queues it"),
            (ready(owed=("a desktop look",)), [RECORD], "owes: a desktop look"),
            (None, None, "unknown"),
        ):
            with self.subTest(pull=pull, comments=comments):
                said = self.stream(Effects(self.path, pull, comments), "queue: watch: #7 turned ready.")
                self.assertEqual(len(said), 1)
                self.assertIn(words, said[0])

    def test_a_ready_pull_request_someone_already_decided(self):
        for pull in (ready(verdict="queue"), ready(labels=("queue",)), ready(verdict="round"), ready(labels=("hold",)),
                     {**ready(), "isDraft": True}):
            with self.subTest(pull=pull):
                self.assertEqual(self.stream(Effects(self.path, pull, [RECORD]), "queue: watch: #7 is ready and unqueued; x"), [])


class Polls(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / quiet.STATE

    def tearDown(self):
        self.dir.cleanup()

    def test_new_bugs_after_the_first_poll_and_not_again_after_a_restart(self):
        effects = Effects(self.path, bugs=[{"number": 10, "title": "old"}])
        q = quiet.Quiet(effects.fx())
        q.poll()
        self.assertEqual(effects.said, [])
        effects.issues.append({"number": 11, "title": "new"})
        q.poll()
        self.assertEqual(len(effects.said), 1)
        self.assertIn("new bug #11", effects.said[0])
        again = Effects(self.path, bugs=effects.issues)
        quiet.Quiet(again.fx()).poll()
        self.assertEqual(again.said, [])

    def test_a_red_main_once_per_head_across_a_restart(self):
        runs = [{"name": "ci", "status": "completed", "conclusion": "success", "head_sha": "a" * 40, "created_at": "1"},
                {"name": "ci", "status": "completed", "conclusion": "failure", "head_sha": "b" * 40, "created_at": "2"},
                {"name": "lint", "status": "in_progress", "conclusion": None, "head_sha": "b" * 40, "created_at": "2"}]
        effects = Effects(self.path, runs=runs)
        quiet.Quiet(effects.fx()).poll()
        self.assertEqual(len(effects.said), 1)
        self.assertIn("main is red: `ci`", effects.said[0])
        again = Effects(self.path, runs=runs)
        quiet.Quiet(again.fx()).poll()
        self.assertEqual(again.said, [])
        runs[1]["conclusion"] = "success"
        green = Effects(self.path, runs=runs)
        quiet.Quiet(green.fx()).poll()
        self.assertEqual(green.said, [])

    def test_a_drain_pass_failing_is_said_once_after_three_in_a_row(self):
        effects = Effects(self.path, drain=["QUIET: the coders' drain pass failed"])
        q = quiet.Quiet(effects.fx())
        for _ in range(quiet.DRAIN_FAILS + 2):
            q.drain()
        self.assertEqual(len(effects.said), 1)
        effects.failed.clear()
        q.drain()
        self.assertEqual(len(effects.said), 1)


if __name__ == "__main__":
    unittest.main()
