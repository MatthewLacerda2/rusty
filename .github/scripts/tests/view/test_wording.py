"""#1876: `events.keep` passes the lines the watch and supervisor really print,
built by their own functions, so a change of wording there fails here first."""

from __future__ import annotations

import unittest

from merging.view import events
from merging.watch import board, ending, supervisor


def prefixed(line: str) -> str:
    return f"queue: {line}"  # `outcomes.say`, as the watch's lines reach the log


class Wording(unittest.TestCase):
    def test_the_watchs_own_lines_pass(self):
        said: list[str] = []
        supervisor.vanished("/wt", said.append)
        before = {1: (True, "a", False), 2: (False, "b", False), 3: (False, "c", False)}
        after = {1: (False, "a", False), 2: (True, "b", False), 4: (False, "d", False)}
        lines = [
            prefixed(ending.handed_back(7, "CI is red.")),
            prefixed(ending.stall_ended(7, "cancelled.")),
            prefixed(board.held_line({"number": 9}, [".github/workflows/ci.yml"])),
            *said,
            *(prefixed(line) for line in board.transitions(before, after, landed=set())),
        ]
        self.assertEqual(len(lines), 8)  # ready, draft, closed, opened as ready
        for line in lines:
            self.assertTrue(events.keep(line, set()), line)

    def test_a_queue_or_draft_opening_does_not(self):
        before = {1: (True, "a", False)}
        after = {1: (True, "a", True), 2: (True, "b", False)}
        for line in board.transitions(before, after, landed=set()):
            self.assertFalse(events.keep(prefixed(line), set()), line)


if __name__ == "__main__":
    unittest.main()
