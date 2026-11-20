#!/usr/bin/env python3
"""#2463: a build tool found before a leg is waited out on a fake clock, and refused only past the limit."""

from __future__ import annotations

import unittest
from unittest import mock

from bench import refuse


class Clock:
    """A fake clock that a sleep moves forward; `sleeps` counts them."""

    def __init__(self):
        self.now, self.sleeps = 0.0, 0

    def sleep(self, s):
        self.now += s
        self.sleeps += 1


def settle(finds) -> tuple[list[str], list[str], Clock]:
    """`refuse.settle` over `finds` (one `busy` read a poll, the last repeated), its lines and the clock."""
    clock, said, reads = Clock(), [], iter(finds)
    last = []

    def find():
        nonlocal last
        last = next(reads, last)
        return last
    with mock.patch.object(refuse, "CLOCK", lambda: clock.now), mock.patch.object(refuse, "SLEEP", clock.sleep):
        found = refuse.settle(find, said.append)
    return found, said, clock


FMT = ["cargo-fmt (PID 4099819)"]


class Settle(unittest.TestCase):
    def test_a_quiet_machine_waits_for_nothing_and_says_nothing(self):
        found, said, clock = settle([[]])
        self.assertEqual((found, said, clock.sleeps), ([], [], 0))

    def test_busy_for_two_polls_then_clear_goes_on_saying_both_lines(self):
        found, said, clock = settle([FMT, FMT, []])
        self.assertEqual(found, [])
        self.assertEqual(clock.sleeps, 3, "one busy poll, then two quiet ones")
        self.assertEqual(len(said), 2)
        self.assertIn("waiting for cargo-fmt (PID 4099819) to end", said[0])
        self.assertIn(f"every {refuse.POLL:.0f} s", said[0])
        self.assertIn(f"quiet after {3 * refuse.POLL:.0f} s", said[1])

    def test_one_quiet_poll_between_two_tools_does_not_end_the_wait(self):
        found, said, clock = settle([FMT, [], FMT, [], []])
        self.assertEqual(found, [])
        self.assertEqual(clock.sleeps, 4, "goes on only after two quiet polls in a row")
        self.assertIn(f"quiet after {4 * refuse.POLL:.0f} s", said[-1])

    def test_no_wait_reads_once_and_never_sleeps(self):
        clock, said = Clock(), []
        with mock.patch.object(refuse, "SLEEP", clock.sleep):
            found = refuse.settle(lambda: FMT, said.append, wait=False)
        self.assertEqual((found, said, clock.sleeps), (FMT, [], 0))

    def test_busy_past_the_limit_is_still_found(self):
        found, said, clock = settle([FMT])
        self.assertEqual(found, FMT)
        self.assertGreaterEqual(clock.now, refuse.LIMIT)
        self.assertLess(clock.now, refuse.LIMIT + refuse.POLL, "no poll past the limit")
        self.assertIn("still found cargo-fmt", said[-1])
        self.assertIn("a build or a bench runs", refuse.busy_refusal(found))


if __name__ == "__main__":
    unittest.main()
