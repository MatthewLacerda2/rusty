#!/usr/bin/env python3
"""#2215: a watch's first listing is compared with the one the last watch told.

What opened or turned ready while no watch ran (the supervisor's ten-minute
gap) is said by the next watch, once; a fresh start says each ready, unlabelled
pull request and nothing else; a different `--since` between two watches says
nothing false; one gone between watches is asked whether it merged. Driven by
the loop tests' fake world, with its state directory kept across watches.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from merging.queue import outcomes
from merging.watch import board, handbacks, kept, loop
from tests.watch import fakes
from tests.watch.fakes import pr

OLD, NEW = "2026-11-01T00:00:00+00:00", "2026-11-15T05:00:00+00:00"


def at(pull: dict, when: str) -> dict:
    return {**pull, "createdAt": when}


def watch(tmp: str, listing: list[dict], verdicts=None, *flags, merged=None) -> list[str]:
    """One watch over one listing (it ends idle); the board lines it said.
    `merged[n]` is what GitHub says of a number gone between watches."""
    world = fakes.World(tmp, [listing], verdicts or {})
    world.fx.was_merged = lambda n: (merged or {}).get(n)
    said = []
    world.fx.say = lambda line, *rest: said.append(line)
    loop.run(world.fx, fakes.opts(*flags, idle=0, poll=30))
    return [s for s in said if s.startswith("watch: #") and "taking" not in s]


def state(tmp: str) -> Path:
    return Path(tmp, handbacks.MEMORY).with_name(kept.BOARD_MEMORY)  # where the watch keeps it


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

    def test_one_gone_between_watches_is_said_as_github_answers_once(self):
        for merged, line in ((True, "watch: #1 merged while no watch listed it."), (False, "watch: #1 closed, not merged."),
                             (None, "watch: #1 closed (merged or not: GitHub did not answer).")):
            with tempfile.TemporaryDirectory() as tmp:
                watch(tmp, [pr(1, draft=True, queued=False), pr(2, draft=True, queued=False)])
                self.assertEqual(watch(tmp, [pr(2, draft=True, queued=False)], merged={1: merged}), [line])
                self.assertEqual(watch(tmp, [pr(2, draft=True, queued=False)]), [])

    def test_one_this_watch_merged_is_not_kept_for_the_next(self):
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [pr(1)], {1: outcomes.MERGED})
            self.assertNotIn(1, (kept.recall(state(tmp)) or ({}, {}))[0])
            self.assertEqual(watch(tmp, []), [])

    def test_a_dry_run_marks_nothing_told(self):
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [pr(1, draft=True, queued=False)])
            watch(tmp, [pr(1, queued=False)], None, "--dry-run")
            self.assertEqual(watch(tmp, [pr(1, queued=False)]), ["watch: #1 turned ready."])


class ANewSince(unittest.TestCase):
    """Every batch starts its supervisor with its own `--since` (the review of PR #2216)."""

    def test_a_later_since_does_not_call_an_old_open_one_closed(self):
        old, new = at(pr(100, draft=True, queued=False), OLD), at(pr(5, draft=True, queued=False), NEW)
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [old], None, "--since", "2026-10-01")
            self.assertEqual(watch(tmp, [old, new], None, "--since", "2026-11-15"), ["watch: #5 opened as a draft."])

    def test_an_earlier_since_does_not_call_an_old_ready_one_opened(self):
        old, new = at(pr(100, queued=False), OLD), at(pr(5, queued=False), NEW)
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [old, new], None, "--since", "2026-11-15")
            self.assertEqual(watch(tmp, [old, new]), [])

    def test_an_old_one_closing_is_not_said_under_a_later_since(self):
        old = at(pr(100, draft=True, queued=False), OLD)
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [old])
            self.assertEqual(watch(tmp, [], None, "--since", "2026-11-15", merged={100: True}), [])


class TheFile(unittest.TestCase):
    def test_an_unreadable_file_is_a_fresh_start(self):
        for text in ('{"heads": {"1": [true, "a"', '{"heads": {"1": "draft"}, "opened": {}}', '[1, 2]',
                     '{"heads": {"x": [true, "a", false]}, "opened": {}}', '{"heads": {"1": [1, "a", 0]}, "opened": {}}',
                     '{"heads": {}}', '{"heads": {}, "opened": {"1": 3}}', ""):
            with tempfile.TemporaryDirectory() as tmp:
                state(tmp).parent.mkdir()
                state(tmp).write_text(text)
                self.assertIsNone(kept.recall(state(tmp)), text)
                self.assertEqual(watch(tmp, [pr(4, queued=False)]), [board.waiting_line(4)], text)

    def test_it_round_trips_and_leaves_no_temporary_behind(self):
        with tempfile.TemporaryDirectory() as tmp:
            heads, opened = {2: (False, "b", True), 1: (True, "a", False)}, {1: OLD, 2: None}
            kept.remember(state(tmp), heads, opened)
            self.assertEqual(kept.recall(state(tmp)), (heads, opened))
            self.assertEqual([p.name for p in state(tmp).parent.iterdir()], [kept.BOARD_MEMORY])

    def test_a_watch_whose_state_cannot_be_written_starts_fresh_and_ends_normally(self):
        with tempfile.TemporaryDirectory() as tmp:
            Path(tmp, handbacks.MEMORY).parent.write_text("")  # the state directory is a file: nothing writes
            world = fakes.World(tmp, [[pr(1, queued=False)]], {})
            said = []
            world.fx.say = lambda line, *rest: said.append(line)
            _, why, status = loop.run(world.fx, fakes.opts(idle=0, poll=30))
        self.assertIn(board.waiting_line(1), said)
        self.assertIn("nothing is ready with its go-ahead", why)  # the idle end, as with a writable directory
        self.assertEqual(status, 0)
        self.assertTrue(any(line.startswith(handbacks.FAILED_LINE) for line in said))
        self.assertEqual(sum(line.startswith(kept.FAILED_LINE) for line in said), 1)  # once a watch

    def test_a_failed_write_removes_the_old_file_and_is_tried_again(self):
        with tempfile.TemporaryDirectory() as tmp:
            watch(tmp, [pr(1, draft=True, queued=False)])
            said = []
            told = kept.Board(state(tmp), say=said.append)
            failing = [True]
            real = kept.remember
            kept.remember = lambda *a: "disk full" if failing[0] else real(*a)
            try:
                told.report([pr(1, queued=False)], set(), None, lambda n: None)
                told.report([pr(1, draft=True, queued=False)], set(), None, lambda n: None)
                self.assertFalse(state(tmp).exists())
                failing[0] = False
                told.keep(set())
            finally:
                kept.remember = real
            self.assertEqual(kept.recall(state(tmp))[0], board.heads([pr(1, draft=True, queued=False)]))
            self.assertEqual(said, [kept.FAILED_LINE + "disk full"])  # two failed listings, said once


if __name__ == "__main__":
    unittest.main()
