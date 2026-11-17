"""#2285: a not-yet-green first is passed over, a manifest change rides, and the take says the line."""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.train import build, group, ride
from merging.watch import loop, order
from tests.train.fakes import Fx, pr
from tests.watch import fakes as watch_fakes


def formed(fx: Fx, line: list[dict], aged=frozenset()):
    members, train, notes, rideable = fx.yard().form(line, aged)
    return [p["number"] for p in members], train.numbers if train else None, notes, rideable


class RunningFirst(unittest.TestCase):
    def test_is_passed_over_and_the_train_forms_from_the_rest(self):
        line = [pr(1), pr(2), pr(3)]
        members, train, notes, rideable = formed(Fx(line, running_own={1}), line)
        self.assertEqual((members, train, rideable), ([2, 3], [2, 3], 2))
        self.assertEqual(notes, ["watch: #1 is passed over this round: its own head's `ci-gate` and `lint-gate` are not both green yet; it keeps its place, and #2, #3 ride without it."])

    def test_one_rider_behind_it_is_taken_alone_and_the_note_says_so(self):
        line = [pr(1), pr(2)]
        members, train, notes, _ = formed(Fx(line, running_own={1}), line)
        self.assertEqual((members, train), ([2], None))
        self.assertTrue(notes[0].endswith("and #2 is taken alone."), notes)

    def test_goes_alone_when_nobody_behind_it_can_ride_and_no_pass_is_noted(self):
        line = [pr(1), pr(2), pr(3)]
        members, train, notes, _ = formed(Fx(line, running_own={1, 2}, clashing={"h3"}), line)
        self.assertEqual((members, train), ([1], None))
        self.assertFalse([n for n in notes if "passed over" in n], notes)
        self.assertIn("#3 waits: it does not merge cleanly onto `main`", notes[1])
        self.assertIn("#1 goes alone: its own head's", notes[-1])
        self.assertEqual(formed(Fx([pr(1)], running_own={1}), [pr(1)])[:2], ([1], None))

    def test_aged_it_goes_first_alone_as_today(self):
        line = [pr(1), pr(2), pr(3)]
        self.assertEqual(formed(Fx(line, running_own={1}), line, aged={1})[:2], ([1], None))

    def test_a_red_first_goes_alone_to_be_handed_back(self):
        line = [pr(1), pr(2), pr(3)]
        members, train, notes, _ = formed(Fx(line, red_own={1}), line)
        self.assertEqual((members, train), ([1], None))
        self.assertIn("#1 goes alone: its own head's `ci-gate` or `lint-gate` is red", notes[0])

    def test_a_bad_description_goes_alone_whatever_its_ci(self):
        line = [pr(1), pr(2)]
        fx = Fx(line, running_own={1})
        fx.ns.claims = lambda n: (n, outcomes.HANDED_BACK, "a stray keyword.") if n == 1 else None
        self.assertEqual(formed(fx, line)[:3], ([1], None, ["watch: #1 goes alone: a stray keyword."]))

    def test_the_pass_keys_on_the_kind_not_the_words(self):
        line = [pr(1), pr(2), pr(3)]
        train = build.Train("main", "main")
        words = str(group.NOT_GREEN)
        self.assertEqual(group.form(line, train, Fx(line).ns.git, lambda p: words if p["number"] == 1 else None)[0], [line[0]])
        self.assertIsInstance(group.NOT_GREEN, group.Unfinished)

    def test_another_refusal_of_the_first_keeps_the_single_take(self):
        alone = pr(1, files=("Makefile",))
        self.assertEqual(formed(Fx([alone, pr(2)]), [alone, pr(2)])[:2], ([1], None))


class Manifests(unittest.TestCase):
    def test_one_rides(self):
        for path in ("Cargo.lock", "Cargo.toml", "crates/core/Cargo.toml"):
            with self.subTest(path=path):
                self.assertIsNone(group.alone([path]))
                line = [pr(1, files=(path,)), pr(2)]
                self.assertEqual(formed(Fx(line), line)[1], [1, 2])

    def test_a_second_never_reaches_the_line(self):  # what holds two apart: `board.lock_waits`
        line = [pr(1, files=("Cargo.lock",)), pr(2, files=("crates/core/Cargo.toml",)), pr(3)]
        self.assertEqual([p["number"] for p in order.ready(line, {}, set(), ())], [1, 3])


class Census(unittest.TestCase):
    def test_counts_the_line_without_asking_claims(self):
        line = [pr(1), pr(2), pr(3, files=("Makefile",)), pr(4), pr(5), pr(6), pr(7)]
        fx = Fx(line, running_own={4}, red_own={5})
        asked = []
        fx.ns.claims = lambda n: asked.append(n)
        members, _, _, rideable = formed(fx, line)
        self.assertEqual((members, rideable), ([1, 2, 6, 7], 4))  # 3 alone, 4 running, 5 red
        self.assertEqual(asked, [1, 2, 4, 5, 6, 7])  # those the walk asked, once each: never 3, never twice

    def test_a_half_is_counted_by_its_own_rule(self):
        line = [pr(1), pr(2), pr(3), pr(4)]
        fx = Fx(line, verdicts=(ride.RED, outcomes.GO))
        yard = fx.yard()
        yard.ride(yard.form(line)[1])
        fx.red_own = {1, 2, 3, 4}  # the half's rule is its claims, not its own CI
        members, _, _, rideable = yard.form(line)
        self.assertEqual(([p["number"] for p in members], rideable), ([1, 2], 2))


class TakeLine(unittest.TestCase):
    def test_names_the_line_and_who_could_ride(self):
        self.assertEqual(loop.taking_census(3, 2), " (3 in line, 2 rideable)")
        self.assertEqual(loop.taking_census(1, None), " (1 in line)")

    def test_a_watch_without_trains_says_the_line(self):
        with tempfile.TemporaryDirectory() as tmp:
            world = watch_fakes.World(tmp, [[watch_fakes.pr(1), watch_fakes.pr(2)]], {1: outcomes.MERGED, 2: outcomes.MERGED})
            said = []
            world.fx.say = lambda *a: said.append(a[0])
            loop.run(world.fx, watch_fakes.opts(idle=1, poll=30))
            self.assertTrue(any(s.startswith("watch: taking #") and "in line)." in s for s in said), said)


if __name__ == "__main__":
    unittest.main()
