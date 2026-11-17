"""#2285: a not-yet-green first is passed over, one manifest change rides, and the take says the line."""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.train import group
from merging.watch import loop
from tests.train.fakes import Fx, pr
from tests.watch import fakes as watch_fakes


def formed(fx: Fx, line: list[dict], aged=frozenset()):
    members, train, notes, rideable = fx.yard().form(line, aged)
    return [p["number"] for p in members], train.numbers if train else None, notes, rideable


class NotGreenFirst(unittest.TestCase):
    def test_is_passed_over_and_the_train_forms_from_the_rest(self):
        line = [pr(1), pr(2), pr(3)]
        members, train, notes, rideable = formed(Fx(line, red_own={1}), line)
        self.assertEqual((members, train, rideable), ([2, 3], [2, 3], 2))
        self.assertIn("#1 is passed over this round: its own head's `ci-gate`", notes[0])

    def test_one_rider_behind_it_is_taken_alone(self):
        line = [pr(1), pr(2)]
        self.assertEqual(formed(Fx(line, red_own={1}), line)[:2], ([2], None))

    def test_goes_alone_when_nobody_behind_it_can_ride(self):
        line = [pr(1), pr(2), pr(3)]
        members, train, notes, _ = formed(Fx(line, red_own={1, 2}, clashing={"h3"}), line)
        self.assertEqual((members, train), ([1], None))
        self.assertIn("#3 waits: it does not merge cleanly onto `main`", notes[2])
        self.assertIn("#1 goes alone: its own head's", notes[-1])
        self.assertEqual(formed(Fx([pr(1)], red_own={1}), [pr(1)])[:2], ([1], None))

    def test_aged_it_goes_first_alone_as_today(self):
        line = [pr(1), pr(2), pr(3)]
        self.assertEqual(formed(Fx(line, red_own={1}), line, aged={1})[:2], ([1], None))

    def test_another_refusal_of_the_first_keeps_the_single_take(self):
        alone = pr(1, files=("Makefile",))
        self.assertEqual(formed(Fx([alone, pr(2)]), [alone, pr(2)])[:2], ([1], None))


class Manifests(unittest.TestCase):
    def test_one_rides_and_a_second_waits(self):
        for path in ("Cargo.lock", "Cargo.toml", "crates/core/Cargo.toml"):
            with self.subTest(path=path):
                self.assertIsNone(group.alone([path]))
                line = [pr(1, files=(path,)), pr(2, files=("Cargo.lock",)), pr(3)]
                members, train, notes, _ = formed(Fx(line), line)
                self.assertEqual(train, [1, 3])
                self.assertIn("#2 waits for a train of its own: it changes Cargo.lock, and #1 in this train", notes[0])

    def test_a_crates_code_is_no_manifest(self):
        self.assertIsNone(group.manifest(["crates/core/src/lib.rs", "crates/core/Cargo.toml.md"]))


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
