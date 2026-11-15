#!/usr/bin/env python3
"""#2215: a watch's first listing is compared with the one the last watch told.

What opened or turned ready while no watch ran (the supervisor's ten-minute
gap) is said by the next watch, once; a fresh start says each ready, unlabelled
pull request and nothing else; an unreadable state file is a fresh start, never
a crash. Driven by the loop tests' fake world, with its state directory kept
across two or three watches.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from merging.queue import outcomes
from merging.watch import board, handbacks, loop, shown
from tests.watch import fakes
from tests.watch.fakes import pr


def watch(tmp: str, listing: list[dict], verdicts=None, *flags) -> list[str]:
    """One watch over one listing (it ends idle); the board lines it said."""
    world = fakes.World(tmp, [listing], verdicts or {})
    said = []
    world.fx.say = lambda line, *rest: said.append(line)
    loop.run(world.fx, fakes.opts(*flags, idle=0, poll=30))
    return [s for s in said if s.startswith("watch: #") and "taking" not in s]


def state(tmp: str) -> Path:
    return Path(tmp, handbacks.MEMORY).with_name(shown.BOARD_MEMORY)  # where the watch keeps it


class AcrossWatches(unittest.TestCase):
    def test_a_pull_request_that_turned_ready_between_two_watches_is_told_by_the_second(self):
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [pr(1, draft=True, queued=False)])
            self.assertEqual(watch(tmp, [pr(1, queued=False), pr(2, draft=True, queued=False)]),
                             ["watch: #1 turned ready.", "watch: #2 opened as a draft."])

    def test_one_already_told_is_not_told_again_by_a_third(self):
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [pr(1, draft=True, queued=False)])
            watch(tmp, [pr(1, queued=False)])
            self.assertEqual(watch(tmp, [pr(1, queued=False)]), [])

    def test_a_fresh_start_tells_the_ready_unlabelled_ones_and_nothing_else(self):
        with tempfile.TemporaryDirectory() as tmp:
            said = watch(tmp, [pr(1, queued=False), pr(2, draft=True, queued=False), pr(3, draft=True)])
        self.assertEqual(said, [board.waiting_line(1)])

    def test_one_closed_meanwhile_is_told_closed_once(self):
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [pr(1, draft=True, queued=False), pr(2, draft=True, queued=False)])
            self.assertEqual(watch(tmp, [pr(2, draft=True, queued=False)]), ["watch: #1 closed, not merged by this watch."])
            self.assertEqual(watch(tmp, [pr(2, draft=True, queued=False)]), [])

    def test_one_this_watch_merged_is_not_kept_for_the_next(self):
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [pr(1)], {1: outcomes.MERGED})
            self.assertNotIn(1, shown.recall(state(tmp)) or {})
            self.assertEqual(watch(tmp, []), [])

    def test_a_dry_run_marks_nothing_told(self):
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [pr(1, draft=True, queued=False)])
            watch(tmp, [pr(1, queued=False)], None, "--dry-run")
            self.assertEqual(watch(tmp, [pr(1, queued=False)]), ["watch: #1 turned ready."])


class TheFile(unittest.TestCase):
    def test_an_unreadable_file_is_a_fresh_start(self):
        for text in ('{"1": [true, "a"', '{"1": "draft"}', '[1, 2]', '{"x": [true, "a", false]}', '{"1": [1, "a", 0]}', ""):
            with tempfile.TemporaryDirectory() as tmp:
                state(tmp).parent.mkdir()
                state(tmp).write_text(text)
                self.assertIsNone(shown.recall(state(tmp)), text)
                self.assertEqual(watch(tmp, [pr(4, queued=False)]), [board.waiting_line(4)], text)

    def test_it_round_trips_and_leaves_no_temporary_behind(self):
        with tempfile.TemporaryDirectory() as tmp:
            heads = {2: (False, "b", True), 1: (True, "a", False)}
            shown.remember(state(tmp), heads)
            self.assertEqual(shown.recall(state(tmp)), heads)
            self.assertEqual([p.name for p in state(tmp).parent.iterdir()], [shown.BOARD_MEMORY])

    def test_a_write_that_fails_costs_the_memory_not_the_watch(self):
        with tempfile.TemporaryDirectory() as tmp:
            blocked = Path(tmp, "file")
            blocked.write_text("")
            shown.remember(Path(blocked, shown.BOARD_MEMORY), {1: (False, "a", False)})  # parent is a file


if __name__ == "__main__":
    unittest.main()
