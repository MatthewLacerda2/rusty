"""#1876: `events.keep` passes the lines the watch and supervisor really print,
built by their own functions, so a change of wording there fails here first."""

from __future__ import annotations

import unittest

from merging.queue import outcomes
from merging.view import events
from merging.watch import board, ending, supervisor
from tests.watch.test_supervisor import fake


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
            prefixed(outcomes.line(8, outcomes.MERGED, "landed with the train abc1234.")),  # `watch.trains.Yard.land`
            prefixed(board.held_line({"number": 9}, [".github/workflows/ci.yml"])),
            *said,
            *(prefixed(line) for line in board.transitions(before, after, landed=set())),
        ]
        self.assertEqual(len(lines), 9)  # ready, draft, closed, opened as ready
        for line in lines:
            self.assertTrue(events.keep(line, set()), line)

    def test_every_line_that_ends_the_supervisors_run_passes_and_no_other(self):
        runs = [fake([], refused="the checkout is dirty."), fake([(0, "no open pull request is left.")], stop_at=1),
                fake([(2, "")]), fake([(0, "relaunch: x"), (ending.STOPPED_STATUS, "")])]
        for fx, said, _ in runs:
            supervisor.run(fx)
            *carried, last = said
            self.assertTrue(events.keep(last, set()), last)
            for line in carried:
                self.assertFalse(events.keep(line, set()), line)

    def test_a_queue_or_draft_opening_does_not(self):
        before = {1: (True, "a", False)}
        after = {1: (True, "a", True), 2: (True, "b", False)}
        for line in board.transitions(before, after, landed=set()):
            self.assertFalse(events.keep(prefixed(line), set()), line)


if __name__ == "__main__":
    unittest.main()
