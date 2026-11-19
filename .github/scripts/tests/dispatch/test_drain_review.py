"""The second review of #2393: a fire time taken at the create, a launched entry dropped with no
slot free, a lock that cannot be opened said and exit 2, and a round ahead taking no new slot."""

from __future__ import annotations

import contextlib
import io
import unittest
from datetime import datetime, timedelta, timezone
from unittest import mock

from dispatch import routine, waiting
from tests.dispatch.coder_fakes import NOW, Board, busy, coder, drafts
from tests.dispatch.test_coders import free, queued
from tests.dispatch.test_drain import line_of, made_after_the_ask


def ticking(step: timedelta):
    """A clock that moves `step` on every read, as a lock wait or a slow create does."""
    times = (NOW + n * step for n in range(1000))
    return lambda: next(times)


class FireTime(unittest.TestCase):
    def test_a_hand_launch_takes_its_fire_time_at_the_create(self):
        fake = Board({})
        fake.open(9)
        clock = ticking(timedelta(seconds=70))
        with mock.patch.object(coder.ghrest, "slug", return_value="o/r"), \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(coder.main(["9"], gh=fake.gh, call=fake.call, env={}, line=fake.line, clock=clock), 0)
        [made] = fake.created()
        self.assertGreater(made["run_once_at"], routine.fire_time(NOW + timedelta(seconds=70)))

    def test_each_create_of_a_pass_takes_its_own_fire_time(self):
        fake = queued(9, 10)
        free(fake, 2)
        cli = __import__("tests").load("coders.py")
        with mock.patch.object(cli.ghrest, "slug", return_value="o/r"), contextlib.redirect_stdout(io.StringIO()):
            cli.main([], gh=fake.gh, call=fake.call, env={}, line=fake.line, clock=ticking(timedelta(minutes=1)))
        first, second = (b["run_once_at"] for b in fake.created())
        self.assertGreater(second, first)
        self.assertGreater(first, routine.fire_time(NOW))


class LaunchedAlready(unittest.TestCase):
    def test_a_pass_with_no_free_slot_still_drops_a_launched_entry(self):
        fake = queued(9, 10)
        made_after_the_ask(fake, 9)
        code, out = fake.coders()
        self.assertEqual((code, fake.created(), line_of(fake)), (0, [], [10]))
        self.assertIn("#9: already launched (rusty #9 x", out)
        self.assertIn("no free slot", out)

    def test_a_hand_launch_of_a_launched_entry_drops_it_and_creates_nothing(self):
        fake = queued(9)
        free(fake, 1)
        made_after_the_ask(fake, 9)
        code, _, err = fake.run("9")
        self.assertEqual((code, fake.created(), line_of(fake)), (0, [], []))
        self.assertIn("#9 was launched from the line already", err)

    def test_over_limit_launches_it_anyway(self):
        fake = queued(9)
        made_after_the_ask(fake, 9)
        self.assertEqual((fake.run("9", "--over-limit")[0], fake.launched()), (0, [9]))


class Unwritable(unittest.TestCase):
    def blocked(self, fake: Board) -> Board:
        """The line's directory is a file: the real `mkdir` fails, nothing mocked."""
        fake.line.parent.parent.mkdir(parents=True, exist_ok=True)
        fake.line.parent.write_text("not a directory")
        return fake

    def test_make_coders_says_what_cannot_be_written_and_exits_2(self):
        fake = self.blocked(queued())
        fake.open(9)
        free(fake, 1)
        entry = {"issue": 9, "brief": "b", "asked": "2026-10-29T14:03:00Z"}  # as a line read before it broke
        with mock.patch.object(waiting, "load", side_effect=lambda _: [dict(entry)]):
            code, out = fake.coders()
        self.assertEqual((code, fake.created()), (2, []))
        self.assertIn(f"cannot write the waiting line {fake.line}", out)

    def test_a_hand_launch_with_an_empty_line_and_a_slot_goes_ahead(self):
        fake = self.blocked(Board({}))
        fake.open(9)
        code, _, err = fake.run("9")
        self.assertEqual((code, fake.launched()), (0, [9]))
        self.assertIn("goes ahead without its lock", err)

    def test_a_hand_launch_at_the_limit_exits_2(self):
        fake = self.blocked(Board({}, routines=busy(1, 2, 3, 4, 5), pulls=drafts(1, 2, 3, 4, 5)))
        fake.open(9)
        code, _, err = fake.run("9")
        self.assertEqual((code, fake.created()), (2, []))
        self.assertIn("could not write the waiting line", err)


class Turn(unittest.TestCase):
    def test_a_round_whose_coder_holds_a_slot_takes_no_new_one(self):
        fake = queued(9, rounds=(9,))
        fake.routines, fake.pulls = busy(1, 2, 3, 9), drafts(1, 2, 3, 9)
        fake.open(12)
        code, _, err = fake.run("12")
        self.assertEqual((code, fake.launched(), line_of(fake)), (0, [12], [9]), err)
        fake.coders()
        self.assertEqual(fake.launched(), [12, 9])

    def test_the_lock_names_no_past_holder(self):
        fake = queued()
        with waiting.held(fake.line):
            pass
        self.assertEqual(fake.line.with_suffix(".lock").read_text(), "")


if __name__ == "__main__":
    unittest.main()
