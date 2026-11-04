"""#1785: the watch rides a train in one take: each member settled, a mismatch stops it with status 9."""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.train import build, land
from merging.watch import ending, loop
from tests.watch.fakes import World, opts, pr


class Yard:
    """A yard that forms one train of everything in line, then rides it to `ends`."""

    def __init__(self, world: World, ends: dict[int, str], halt=None):
        self.world, self.ends, self.halt, self.rode = world, ends, halt, []

    def form(self, ranked):
        if len(ranked) < 2:
            return [ranked[0]], None, []
        train = build.Train("main", "main", [build.Car(p["number"], p["headRefOid"], "t", "c") for p in ranked])
        return ranked, train, ["watch: a note."]

    def ride(self, train, tick=None):
        self.rode.append(train.numbers)
        for n in train.numbers:
            if self.ends[n] == outcomes.MERGED:
                self.world.verdicts[n] = outcomes.MERGED
                self.world.taken.append(n)  # the fake lists what merged no more
        return [(n, self.ends[n], "why") for n in train.numbers], self.halt


def run(ends: dict[int, str], halt=None):
    with tempfile.TemporaryDirectory() as tmp:
        world = World(tmp, [[pr(n) for n in ends]], {n: outcomes.MERGED for n in ends})
        world.fx.trains = yard = Yard(world, ends, halt)
        results, why, status = loop.run(world.fx, opts(idle=1, poll=30))
        return yard, results, why, status


class Loop(unittest.TestCase):
    def test_a_green_train_merges_its_members_in_one_take(self):
        yard, results, why, status = run({1: outcomes.MERGED, 2: outcomes.MERGED, 3: outcomes.MERGED})
        self.assertEqual((yard.rode, status), ([[1, 2, 3]], 0))
        self.assertEqual([state for _, state, _ in results], [outcomes.MERGED] * 3)

    def test_members_back_in_line_are_taken_again(self):
        yard, results, why, status = run({1: outcomes.MERGED, 2: outcomes.REQUEUED})
        self.assertEqual(yard.rode, [[1, 2]])  # then #2 alone, today's path
        self.assertEqual(results[-1][:2], (2, outcomes.MERGED))
        self.assertEqual(status, 0)  # nothing was handed back

    def test_github_down_mid_landing_settles_what_landed_and_stops_as_the_machine(self):
        yard, results, why, status = run({1: outcomes.MERGED, 2: outcomes.STOPPED, 3: outcomes.REQUEUED})
        self.assertEqual(status, ending.MACHINE_STATUS)  # the supervisor retries it, with its backoff
        self.assertEqual([r[:2] for r in results], [(1, outcomes.MERGED), (2, outcomes.STOPPED), (3, outcomes.REQUEUED)])

    def test_a_tree_mismatch_stops_the_watch_with_its_own_status(self):
        halt = f"{land.MISMATCH}: look at main."
        yard, results, why, status = run({1: outcomes.MERGED, 2: outcomes.MERGED}, (halt, ending.MISMATCH_STATUS))
        self.assertEqual((why, status), (halt, ending.MISMATCH_STATUS))


if __name__ == "__main__":
    unittest.main()
