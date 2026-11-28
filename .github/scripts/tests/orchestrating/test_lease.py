"""#2667: one orchestrator at a time, on a branch both sides push to without force."""

from __future__ import annotations

import unittest

from orchestrating import lease
from tests.orchestrating.fakes import CLOUD, LOCAL, OTHER_CLOUD, Remote


class Taking(unittest.TestCase):
    def setUp(self):
        self.remote = Remote(self)
        self.cloud, self.local = self.remote.side("a", CLOUD), self.remote.side("b", LOCAL)

    def test_a_lock_never_taken_is_taken_and_names_its_holder(self):
        self.assertEqual(self.cloud.take(), (None, []))
        found = lease.read(self.local.git)
        self.assertEqual((found.holder, found.at), (CLOUD, self.remote.clock.now))

    def test_the_other_side_is_refused_while_the_holder_is_fresh_and_told_who(self):
        self.cloud.take()
        self.remote.clock.advance(44)
        refused, _ = self.local.take()
        self.assertIn(f"held by {CLOUD}, renewed 44 minute(s) ago", refused)
        self.assertEqual(lease.read(self.local.git).holder, CLOUD)

    def test_and_the_other_way_round(self):
        self.local.take()
        refused, _ = self.cloud.take()
        self.assertIn(f"held by {LOCAL}", refused)

    def test_a_holder_that_stopped_renewing_is_taken_over_once_stale(self):
        self.cloud.take()
        self.remote.clock.advance(lease.STALE.total_seconds() / 60)
        self.assertEqual(self.local.take()[0], None)
        self.assertEqual(lease.read(self.cloud.git).holder, LOCAL)

    def test_renewing_keeps_a_holder_fresh_past_the_stale_age(self):
        self.cloud.take()
        for _ in range(6):
            self.remote.clock.advance(lease.RENEW.total_seconds() / 60)
            self.assertIsNone(self.cloud.renew())
        self.assertIsNotNone(self.local.take()[0])

    def test_a_released_lock_is_free_and_says_who_released_it(self):
        self.cloud.take()
        self.assertIsNone(self.cloud.release())
        found = lease.read(self.local.git)
        self.assertEqual((found.holder, found.by), (None, CLOUD))
        self.assertEqual(self.local.take()[0], None)

    def test_a_side_that_lost_the_lock_can_neither_renew_nor_release_it(self):
        self.cloud.take()
        self.remote.clock.advance(60)
        self.local.take()
        self.assertIn(f"no longer {CLOUD}'s: held by {LOCAL}", self.cloud.renew())
        self.assertIn(f"held by {LOCAL}", self.cloud.release())
        self.assertEqual(lease.read(self.local.git).holder, LOCAL)


class Racing(unittest.TestCase):
    def test_two_takes_from_one_tip_leave_one_holder_and_the_loser_names_it(self):
        remote = Remote(self)
        first, second = remote.side("a", CLOUD), remote.side("b", OTHER_CLOUD)
        first.take()
        first.release()
        pushes = []

        def racing(*args, stdin=None):
            if args[0] == "push" and not pushes:
                pushes.append(first.take())  # the first side gets in between the read and the push
            return second_git(*args, stdin=stdin)
        second_git = second.git
        before, after = lease.take(racing, OTHER_CLOUD, remote.clock.now)
        self.assertIsNone(after)
        self.assertEqual(before.holder, CLOUD)
        self.assertEqual(lease.read(second_git).holder, CLOUD)


class Reading(unittest.TestCase):
    def test_a_message_it_cannot_read_is_a_free_lock(self):
        self.assertIsNone(lease.parse("t", lease.EMPTY_TREE, "something else\n").holder)

    def test_a_holder_with_spaces_is_read_back_whole(self):
        found = lease.parse("t", lease.EMPTY_TREE, "orchestrator lock: held by local@a b hand at 2026-11-28T06:00:00Z")
        self.assertEqual(found.holder, "local@a b hand")

    def test_an_unreachable_origin_raises_rather_than_reading_free(self):
        def down(*_args, stdin=None):
            return 128, "", "fatal: unable to access 'https://github.com/': Could not resolve host"
        with self.assertRaises(lease.Unreachable):
            lease.read(down)


if __name__ == "__main__":
    unittest.main()
