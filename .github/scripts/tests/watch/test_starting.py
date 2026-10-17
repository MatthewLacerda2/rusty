#!/usr/bin/env python3
"""#928: how a watch starts and ends — one at a time (the lock), within what a
background command survives (the budget), `--since`, the board reported during
a CI wait, and the `resume:` last line.
"""

from __future__ import annotations

import tempfile
import unittest
import unittest.mock
from datetime import datetime, timezone
from pathlib import Path

from merging.queue import cli, outcomes, waiting, world
from merging.verdict import github
from merging.watch import board, ending, lock, loop
from tests.watch import fakes


class Lock(unittest.TestCase):
    def test_a_second_holder_is_refused_and_named_until_the_first_lets_go(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, ".git", lock.LOCK)
            first, _ = lock.hold(path)
            second, holder = lock.hold(path)
            self.assertIsNone(second)
            self.assertTrue(holder.isdigit())
            first.close()
            again, _ = lock.hold(path)
            self.assertIsNotNone(again)
            again.close()

    def test_main_refuses_a_second_watch(self):
        with tempfile.TemporaryDirectory() as tmp:
            first, _ = lock.hold(cli.lock_path(tmp))
            with unittest.mock.patch("builtins.print") as printed:
                self.assertEqual(cli.main(["--watch", "--no-check", f"--root={tmp}"]), lock.LOCKED_STATUS)
            first.close()
        self.assertIn("another watch holds", printed.call_args_list[0].args[0])


class Budget(unittest.TestCase):
    def test_for_and_deadline_must_fit_in_two_hours(self):
        self.assertIsNone(ending.over_budget(70, 45))
        self.assertIsNone(ending.over_budget(None, 600))
        self.assertIn("--for 75", ending.over_budget(110, 45))
        with self.assertRaises(SystemExit), unittest.mock.patch("sys.stderr"):
            cli.parse(["--watch", "--for", "50", "--deadline", "80"])


class Since(unittest.TestCase):
    def test_only_the_batch_s_own_are_on_the_board(self):
        old, new = fakes.pr(1), fakes.pr(2)
        old["createdAt"], new["createdAt"] = "2026-10-15T10:00:00Z", "2026-10-16T20:00:00Z"
        since = cli.parse(["--watch", "--since", "2026-10-16T00:00+00:00"]).since
        self.assertEqual(board.recent([old, new, fakes.pr(3)], since), [new, fakes.pr(3)])
        self.assertEqual(board.recent([old], None), [old])
        self.assertIsNotNone(cli.since("2026-10-16").tzinfo)
        with self.assertRaises(SystemExit), unittest.mock.patch("sys.stderr"):
            cli.parse(["7", "--since", "2026-10-16"])


class Tick(unittest.TestCase):
    def test_a_pull_request_turning_ready_mid_take_is_reported_then(self):
        with tempfile.TemporaryDirectory() as tmp:
            scripted = fakes.World(tmp, [[fakes.pr(1), fakes.pr(2, draft=True)]], {1: outcomes.MERGED, 2: outcomes.MERGED})
            said = []
            scripted.fx.say = said.append

            def turn(number, tick):
                if number == 1:
                    scripted.listings = [[fakes.pr(1), fakes.pr(2)]]
                    tick()
                    said.append("take ends")
                return fakes.World.turn(scripted, number)
            scripted.fx.turn = turn
            loop.run(scripted.fx, fakes.opts())
        self.assertLess(said.index("watch: #2 turned ready."), said.index("take ends"))
        self.assertEqual(said.count("watch: #2 turned ready."), 1)

    def test_the_ci_wait_ticks_once_per_poll(self):
        ticks, states = [], iter([(outcomes.WAIT, ["not yet"]), (outcomes.STOP, ["gone"])])
        with unittest.mock.patch("merging.queue.pulls.look", lambda n: {}), \
                unittest.mock.patch("merging.queue.polling.head_state", lambda *a: next(states)), \
                unittest.mock.patch("time.sleep"), unittest.mock.patch("builtins.print"):
            waiting.wait_for("o/r", 1, "a", "b", 60, 30, lambda: ticks.append(1))
        self.assertEqual(ticks, [1])


class Resume(unittest.TestCase):
    def test_the_last_line_is_the_command_to_start_it_again(self):
        line = ending.resume("/r/my wt", ".github/scripts/merge-queue.py", ["--watch", "--for", "70"])
        self.assertEqual(line, "watch: resume: cd '/r/my wt' && python3 .github/scripts/merge-queue.py --watch --for 70")

    def test_main_ends_on_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            scripted = fakes.World(tmp, [[]], {})
            with unittest.mock.patch.object(world, "effects", lambda repo, opts: scripted.fx), \
                    unittest.mock.patch.object(github, "gh", lambda *a: {"nameWithOwner": "o/r"}), \
                    unittest.mock.patch("builtins.print") as printed:
                self.assertEqual(cli.main(["--watch", "--no-check", f"--root={tmp}"]), 0)
        self.assertTrue(printed.call_args_list[-1].args[0].startswith(f"queue: {ending.RESUME} cd "))


if __name__ == "__main__":
    unittest.main()
