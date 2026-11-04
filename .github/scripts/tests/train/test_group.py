"""#1785: who rides a train, who goes alone, who waits ([`watch.trains.Yard.form`], [`train.group`])."""

from __future__ import annotations

import unittest

from merging.train import group
from tests.train.fakes import Fx, pr


def formed(fx: Fx, line: list[dict]):
    members, train, notes = fx.yard().form(line)
    return [p["number"] for p in members], train.numbers if train else None, notes


class Forming(unittest.TestCase):
    def test_a_conflict_with_the_train_passes_over_and_keeps_its_place(self):
        line = [pr(1), pr(2), pr(3)]
        fx = Fx(line, clashing={"h2"})
        self.assertEqual(formed(fx, line)[:2], ([1, 3], [1, 3]))
        self.assertIn("#2 waits: it conflicts with the train so far", formed(fx, line)[2][0])
        # Next round the line is the watch's, unchanged: #2 leads it and is taken.
        self.assertEqual(formed(Fx([pr(2), pr(4)]), [pr(2), pr(4)])[1], [2, 4])

    def test_the_cap_holds(self):
        line = [pr(n) for n in range(1, 8)]
        self.assertEqual(formed(Fx(line), line)[1], [1, 2, 3, 4])
        self.assertEqual(group.CAP, 4)

    def test_a_merge_path_or_manifest_change_goes_alone_first_and_waits_behind(self):
        for path in (".github/scripts/merging/watch/loop.py", ".github/workflows/ci.yml", "Cargo.lock", "Makefile", "tools/lint/src/main.rs"):
            with self.subTest(path=path):
                alone = pr(1, files=(path,))
                self.assertEqual(formed(Fx([alone, pr(2)]), [alone, pr(2)])[:2], ([1], None))
                members, train, notes = formed(Fx([pr(2), alone, pr(3)]), [pr(2), alone, pr(3)])
                self.assertEqual(train, [2, 3])
                self.assertIn(f"#1 waits for a train of its own: it changes {path}", notes[0])

    def test_red_or_missing_own_ci_waits(self):
        line = [pr(1), pr(2), pr(3)]
        members, train, notes = formed(Fx(line, red_own={2}), line)
        self.assertEqual(train, [1, 3])
        self.assertIn("`ci-gate` and `lint-gate` are not both green", notes[0])

    def test_a_first_that_cannot_ride_is_todays_path(self):
        line = [pr(1), pr(2)]
        self.assertEqual(formed(Fx(line, clashing={"h1"}), line)[:2], ([1], None))  # conflicts with `main`
        self.assertEqual(formed(Fx(line, red_own={1}), line)[:2], ([1], None))
        bot = {**pr(1), "author": {"login": "dependabot[bot]"}}
        self.assertEqual(formed(Fx([bot, pr(2)]), [bot, pr(2)])[:2], ([1], None))

    def test_a_train_of_one_is_todays_path(self):
        line = [pr(1), pr(2)]
        self.assertEqual(formed(Fx(line, clashing={"h2"}), line)[:2], ([1], None))

    def test_a_head_red_in_a_train_goes_alone_until_it_moves(self):
        line = [pr(1), pr(2)]
        fx = Fx(line)
        yard = fx.yard()
        yard.red[1] = "h1"
        self.assertIsNone(yard.form(line)[1])
        self.assertEqual(yard.form([pr(1, head="new"), pr(2)])[1].numbers, [1, 2])


if __name__ == "__main__":
    unittest.main()
