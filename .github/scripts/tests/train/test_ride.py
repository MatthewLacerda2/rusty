"""#1785: a train's ride: green lands all and checks `main`'s tree, red splits, the rest goes back in line."""

from __future__ import annotations

import unittest

from merging.queue import landing, outcomes
from merging.train import land, ride
from tests.train.fakes import Fx, pr


LINE = [pr(1), pr(2), pr(3), pr(4)]


def go(fx: Fx, line=LINE):
    yard = fx.yard()
    train = yard.form(line)[1]
    results, halt = yard.ride(train)
    return yard, [(n, state) for n, state, _ in results], halt


class Riding(unittest.TestCase):
    def test_green_lands_every_member_in_order_and_checks_the_tree(self):
        fx = Fx(LINE)
        _, results, halt = go(fx)
        self.assertEqual((fx.merged, halt), ([1, 2, 3, 4], None))
        self.assertEqual(results, [(n, outcomes.MERGED) for n in (1, 2, 3, 4)])
        self.assertEqual((fx.pushed, len(fx.deleted)), ([[1, 2, 3, 4]], 1))

    def test_a_tree_mismatch_stops_the_watch(self):
        fx = Fx(LINE)
        fx.tree_override = "something else"
        self.assertIn(land.MISMATCH, go(fx)[2])

    def test_a_merge_from_elsewhere_while_landing_stops_the_watch(self):
        fx = Fx(LINE)
        real = fx.squash

        def squash(pull, sha):
            if pull["number"] == 2:
                fx.main_moved = 1  # somebody merged between the calls
            return real(pull, sha)

        fx.ns.squash = squash
        self.assertIn("something else merged while it landed", go(fx)[2])

    def test_red_splits_and_marks_its_heads(self):
        fx = Fx(LINE, verdicts=(ride.RED, outcomes.GO))
        yard, results, halt = go(fx)
        self.assertEqual(results, [(n, outcomes.REQUEUED) for n in (1, 2, 3, 4)])
        self.assertEqual(yard.waiting, [[(1, "h1"), (2, "h2")], [(3, "h3"), (4, "h4")]])
        self.assertEqual(yard.red, {n: f"h{n}" for n in (1, 2, 3, 4)})
        # The halves come first, whatever the line says, and a green one lands.
        train = yard.form(LINE)[1]
        self.assertEqual(train.numbers, [1, 2])
        yard.ride(train)
        self.assertEqual(fx.merged, [1, 2])
        self.assertEqual(yard.form(LINE[2:])[1].numbers, [3, 4])

    def test_a_red_half_of_one_is_todays_path(self):
        fx = Fx(LINE[:2], verdicts=(ride.RED,))
        yard, _, _ = go(fx, LINE[:2])
        self.assertEqual(yard.form(LINE[:2])[:2], ([LINE[0]], None))  # handed back by the take, as today
        self.assertEqual(yard.form(LINE[:2])[:2], ([LINE[1]], None))

    def test_a_half_whose_head_moved_is_dropped(self):
        fx = Fx(LINE, verdicts=(ride.RED,))
        yard, _, _ = go(fx)
        moved = [pr(1, head="new"), pr(2, head="new"), *LINE[2:]]
        self.assertEqual(yard.form(moved)[1].numbers, [3, 4])

    def test_a_stalled_train_splits_without_marking_anyone_red(self):
        yard, results, _ = go(Fx(LINE, verdicts=(ride.STALLED,)))
        self.assertEqual((yard.red, len(yard.waiting)), ({}, 2))

    def test_a_stale_train_rebuilds_on_the_new_main(self):
        fx = Fx(LINE)
        fx.main_moved = 1
        _, results, halt = go(fx)
        self.assertEqual((fx.merged, halt), ([], None))
        self.assertEqual(results, [(n, outcomes.REQUEUED) for n in (1, 2, 3, 4)])

    def test_a_merge_lost_in_transit_ends_the_landing_and_the_rest_go_back_in_line(self):
        fx = Fx(LINE)
        fx.squashes[2] = landing.UNKNOWN
        _, results, halt = go(fx)
        self.assertEqual(results, [(1, outcomes.MERGED), (2, outcomes.HANDED_BACK), (3, outcomes.REQUEUED), (4, outcomes.REQUEUED)])
        self.assertIsNone(halt)  # `main` holds #1 alone, a tree the train built
        fx = Fx(LINE)
        fx.squashes[2] = outcomes.HANDED_BACK  # refused
        self.assertEqual(go(fx)[1][2:], [(3, outcomes.REQUEUED), (4, outcomes.REQUEUED)])

    def test_a_member_that_moved_does_not_land_and_nor_do_those_after_it(self):
        fx = Fx(LINE)
        fx.by[3] = pr(3, head="pushed")
        _, results, halt = go(fx)
        self.assertEqual((fx.merged, halt), ([1, 2], None))
        self.assertEqual(results[2:], [(3, outcomes.REQUEUED), (4, outcomes.REQUEUED)])

    def test_a_dry_run_or_a_failed_local_check_pushes_nothing(self):
        fx = Fx(LINE)
        fx.ns.dry_run = True
        self.assertEqual({state for _, state in go(fx)[1]}, {outcomes.DRY})
        fx = Fx(LINE)
        fx.broken = ["cargo check failed"]
        yard, results, _ = go(fx)
        self.assertEqual((fx.pushed, len(yard.waiting)), ([], 2))


if __name__ == "__main__":
    unittest.main()
