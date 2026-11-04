"""#1785's review: a landing that fails partway is settled, checked and stopped, never a traceback.

GitHub failing mid-landing (out of retries, or a `gh` refusal that exits),
`main` unreadable before or after landing, a description that gained a stray
keyword while the train ran, "Base branch was modified", and a green train that
went stale: each ends where [`watch.trains.Yard`] says, with what landed kept.
"""

from __future__ import annotations

import unittest

from merging.queue import landing, outcomes
from merging.train import land, ride
from merging.verdict import github
from merging.watch import ending
from tests.train.fakes import Fx, pr


LINE = [pr(1), pr(2), pr(3), pr(4)]


def go(fx: Fx, line=LINE):
    yard = fx.yard()
    results, halt = yard.ride(yard.form(line)[1])
    return yard, [(n, state) for n, state, _ in results], halt


def failing_look(fx: Fx, at: int, down: BaseException):
    def look(n):
        if n == at:
            raise down
        return fx.by[n]
    return look


class GitHubDown(unittest.TestCase):
    def test_a_look_out_of_retries_after_the_first_merge_settles_and_checks(self):
        for down in (github.Unreachable("gh api: timeout"), SystemExit("mergeable: gh api: HTTP 404")):
            fx = Fx(LINE)
            fx.ns.look = failing_look(fx, 2, down)
            _, results, halt = go(fx)
            self.assertEqual(results, [(1, outcomes.MERGED), (2, outcomes.STOPPED), (3, outcomes.REQUEUED), (4, outcomes.REQUEUED)])
            self.assertEqual((fx.merged, halt, fx.main_reads), ([1], None, 2))  # the tree check ran, and passed

    def test_a_squash_that_raised_may_have_merged(self):
        fx = Fx(LINE)

        def squash(pull, sha):
            if pull["number"] == 2:
                fx.merged.append(2)  # it went through before GitHub failed
                raise github.Unreachable("gh api: timeout")
            return Fx.squash(fx, pull, sha)

        fx.ns.squash = squash
        _, results, halt = go(fx)
        self.assertEqual((results[1], halt), ((2, outcomes.STOPPED), None))

    def test_main_unreadable_before_landing_lands_nothing_and_reports_no_mismatch(self):
        fx = Fx(LINE)
        fx.unknown_at = {0}
        _, results, halt = go(fx)
        self.assertEqual((fx.merged, halt), ([], None))
        self.assertEqual(results, [(1, outcomes.STOPPED), *((n, outcomes.REQUEUED) for n in (2, 3, 4))])

    def test_main_unreadable_after_landing_stops_as_the_machine_not_a_mismatch(self):
        fx = Fx(LINE)
        fx.unknown_at = {1}
        _, results, halt = go(fx)
        self.assertEqual(fx.merged, [1, 2, 3, 4])
        self.assertEqual(halt[1], ending.MACHINE_STATUS)
        self.assertNotIn(land.MISMATCH, halt[0])


class Landing(unittest.TestCase):
    def test_a_stray_keyword_added_while_the_train_ran_does_not_land(self):
        fx = Fx(LINE)
        fx.by[3] = {**pr(3), "body": "This would close #12 by accident."}
        _, results, _ = go(fx)
        self.assertEqual((fx.merged, results[2:]), ([1, 2], [(3, outcomes.REQUEUED), (4, outcomes.REQUEUED)]))

    def test_base_branch_modified_goes_back_in_line_not_handed_back(self):
        fx = Fx(LINE)
        fx.squashes[2] = landing.BASE_MOVED
        _, results, halt = go(fx)
        self.assertEqual(results, [(1, outcomes.MERGED), *((n, outcomes.REQUEUED) for n in (2, 3, 4))])
        self.assertIsNone(halt)

    def test_a_stale_green_half_is_rebuilt_whole_and_marked_red_no_more(self):
        fx = Fx(LINE, verdicts=(ride.RED, outcomes.GO))
        yard, _, _ = go(fx)  # red: halves [1, 2] and [3, 4]
        fx.main_moved = 1
        results = yard.ride(yard.form(LINE)[1])[0]
        self.assertEqual({state for _, state, _ in results}, {outcomes.REQUEUED})
        self.assertNotIn(1, yard.red)
        self.assertEqual(yard.form(LINE)[1].numbers, [1, 2])  # rebuilt, not #1 alone


if __name__ == "__main__":
    unittest.main()
