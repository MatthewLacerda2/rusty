"""#2382, the review of #2393: no path starts a second coder on an issue that has one, a hand
launch never jumps the line, and a held line is waited on for a bounded time."""

from __future__ import annotations

import fcntl
import unittest
from unittest import mock

from dispatch import remote, waiting
from tests.dispatch.coder_fakes import ROUND, brief_on
from tests.dispatch.test_coders import free, queued


def line_of(fake) -> list[int]:
    return [e["issue"] for e in waiting.load(fake.line)]


def made_after_the_ask(fake, number: int) -> None:
    fake.routines = {"data": [*fake.routines["data"],
                              {"name": f"rusty #{number} x", "enabled": True, "created_at": "2026-10-29T14:30:00Z"}]}


class NeverTwice(unittest.TestCase):
    def test_a_pass_dying_after_its_first_create_launches_nothing_twice(self):
        fake = queued(9, 10)
        free(fake, 2)
        fake.create = [(200, None), remote.RemoteError("the create timed out")]
        code, out = fake.coders()
        self.assertEqual((code, fake.launched(), line_of(fake)), (2, [9, 10], [10]))  # #10's create raised
        self.assertIn("timed out", out)
        fake.create = (200, None)
        self.assertEqual(fake.coders()[0], 0)
        self.assertEqual((fake.launched(), line_of(fake)), ([9, 10, 10], []))

    def test_an_entry_with_a_routine_made_after_its_ask_is_dropped_not_launched(self):
        fake = queued(9)
        free(fake, 1)
        made_after_the_ask(fake, 9)
        code, out = fake.coders()
        self.assertEqual((code, fake.created(), line_of(fake)), (0, [], []))
        self.assertIn("#9: already launched (rusty #9 x", out)

    def test_a_routine_made_before_the_ask_is_the_coder_a_round_replaces(self):
        fake = queued(9, rounds=(9,))
        free(fake, 1)
        fake.routines["data"].append({"name": "rusty #9 x", "enabled": True, "created_at": "2026-10-29T13:00:00Z"})
        fake.coders()
        self.assertEqual(fake.launched(), [9])

    def test_a_line_that_cannot_be_written_after_a_launch_exits_2_then_drops_it(self):
        fake = queued(9)
        free(fake, 1)
        with mock.patch.object(waiting.files, "write", return_value="read-only file system"):
            code, out = fake.coders()
        self.assertEqual((code, fake.launched()), (2, [9]))
        self.assertIn("could not write the waiting line", out)
        _, out = fake.coders()
        self.assertEqual((fake.launched(), line_of(fake)), ([9], []))
        self.assertIn("already launched", out)

    def test_a_hand_launch_that_cannot_clear_the_line_exits_2(self):
        fake = queued(9)
        with mock.patch.object(waiting.files, "write", return_value="read-only file system"):
            code, _, err = fake.run("9", "--over-limit")
        self.assertEqual((code, fake.launched()), (2, [9]))
        self.assertIn("could not take #9 out of the waiting line", err)

    def test_no_free_slot_reads_no_issue_in_line(self):
        fake = queued(9)
        read = []
        gh = fake.gh
        fake.gh = lambda path: read.append(path) or gh(path)
        _, out = fake.coders()
        self.assertEqual([p for p in read if "/issues/9" in p], [])
        self.assertIn("no free slot", out)


class TheLine(unittest.TestCase):
    def test_a_hand_launch_with_a_free_slot_waits_behind_the_line(self):
        fake = queued(10, rounds=(10,))
        free(fake, 1)
        fake.open(11)
        code, _, err = fake.run("11")
        self.assertEqual((code, fake.created(), line_of(fake)), (0, [], [10, 11]))
        self.assertIn("1 ahead in the waiting line: #10 (round)", err)
        self.assertEqual((fake.run("11", "--no-wait")[0], fake.created()), (1, []))

    def test_a_hand_launch_within_the_free_slots_goes(self):
        fake = queued(10)
        free(fake, 2)
        fake.open(11, comments=[brief_on(11)])
        self.assertEqual((fake.run("11")[0], fake.launched(), line_of(fake)), (0, [11], [10]))

    def test_the_first_in_line_launches_by_hand(self):
        fake = queued(10, 11)
        free(fake, 1)
        self.assertEqual((fake.run("10")[0], fake.launched(), line_of(fake)), (0, [10], [11]))


class Held(unittest.TestCase):
    def test_a_held_line_is_waited_on_then_named(self):
        fake = queued()
        fake.line.parent.mkdir(parents=True, exist_ok=True)
        with open(fake.line.with_suffix(".lock"), "a+") as other:
            fcntl.flock(other, fcntl.LOCK_EX)
            other.write("pid 4242 (coders.py), since then\n")
            other.flush()
            with mock.patch.object(waiting, "WAIT_SECONDS", 0):
                code, _, err = fake.run("9", "--over-limit")
        self.assertEqual((code, fake.created()), (2, []))
        self.assertIn("still held by pid 4242 (coders.py)", err)

    def test_a_dry_run_takes_no_lock_and_makes_no_directory(self):
        fake = queued()
        fake.open(9)
        for run in (lambda: fake.run("9", "--dry-run"), lambda: fake.coders("--dry-run")):
            run()
            self.assertFalse(fake.line.parent.exists())

    def test_a_dry_run_says_would_for_a_drop(self):
        fake = queued(9)
        fake.open(9, state="closed")
        free(fake, 1)
        _, out = fake.coders("--dry-run")
        self.assertIn("#9: would be dropped from the line", out)
        self.assertEqual(line_of(fake), [9])


if __name__ == "__main__":
    unittest.main()
