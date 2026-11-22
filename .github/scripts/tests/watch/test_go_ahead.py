#!/usr/bin/env python3
"""#2628: which ready pull request has the go-ahead — the label, the verdict, `hold`, `Needs:`.

Must take: a `queue` label (whatever the verdict), a `queue` verdict with no
label, a pull request with no standing read and the label (as before #2628).
Must not: a `round` or `hold` verdict with no label, a `hold` label (whatever
else), anything owed, a pull request with no standing read and no label.
"""

from __future__ import annotations

import unittest
from pathlib import Path

from merging.watch import board, order

MERGING = Path(__file__).resolve().parents[2] / "merging"


def pull(number: int = 1, labels: tuple[str, ...] = (), verdict: str | None = None, owed: tuple[str, ...] = (),
         read: bool = True) -> dict:
    shaped = {"number": number, "isDraft": False, "headRefOid": f"h{number}", "baseRefName": "main",
              "labels": [{"name": n} for n in labels]}
    if read:
        shaped["standing"] = {"verdict": verdict, "said": verdict, "on": "abc1234", "owed": list(owed)}
    return shaped


class GoAhead(unittest.TestCase):
    def test_taken(self):
        for case in (
            pull(labels=("queue",)),
            pull(labels=("queue",), verdict="round"),  # the override by hand
            pull(labels=("queue",), verdict="hold"),
            pull(verdict="queue"),
            pull(labels=("queue",), read=False),  # a record with no verdict: as before
        ):
            with self.subTest(case=case):
                self.assertTrue(board.queued(case))
                self.assertEqual([p["number"] for p in order.ready([case], {}, set(), ())], [1])

    def test_not_taken(self):
        for case in (
            pull(),
            pull(verdict="round"),
            pull(verdict="hold"),
            pull(labels=("hold",), verdict="queue"),
            pull(labels=("hold", "queue")),
            pull(labels=("queue",), verdict="queue", owed=("a desktop measurement",)),
            pull(read=False),
        ):
            with self.subTest(case=case):
                self.assertFalse(board.queued(case))
                self.assertEqual(order.ready([case], {}, set(), ()), [])

    def test_a_verdict_arriving_is_movement_said_once(self):
        before = board.heads([pull(verdict=None)])
        after = board.heads([pull(verdict="queue")])
        self.assertEqual(board.transitions(before, after, set()), ["watch: #1 queued."])


class Rereads(unittest.TestCase):
    """A re-read without its standing reads a verdict-queued pull request as unqueued."""

    def test_every_reread_that_asks_the_go_ahead_carries_the_verdict(self):
        bare = [f"{path.relative_to(MERGING)}: {line.strip()}" for path in sorted(MERGING.rglob("*.py"))
                for line in path.read_text().splitlines() if "queued(pulls.look(" in line or "look=pulls.look," in line]
        self.assertEqual(bare, [], "re-read with `queue.pulls.look_standing`, which attaches the verdict `board.queued` reads")

    def test_the_check_finds_a_bare_reread(self):
        self.assertIn("queued(pulls.look(", "if not board.queued(pulls.look(number)):")


if __name__ == "__main__":
    unittest.main()
