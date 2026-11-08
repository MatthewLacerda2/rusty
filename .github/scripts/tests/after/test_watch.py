"""The watch hands each merge to the mutation hook and pumps it (#1934)."""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.watch import loop
from tests.watch import fakes


class Hook:
    def __init__(self) -> None:
        self.landed: list[int] = []
        self.pumps = 0

    def merged(self, number: int) -> None:
        self.landed.append(number)

    def pump(self) -> None:
        self.pumps += 1


class Watch(unittest.TestCase):
    def run_watch(self, verdicts: dict[int, str]) -> Hook:
        hook = Hook()
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, [[fakes.pr(n, head=f"pushed{n}") for n in verdicts]], verdicts)
            world.fx.mutants = hook
            loop.run(world.fx, fakes.opts(idle=1, poll=30))
        return hook

    def test_only_a_merge_reaches_the_hook_and_each_pass_pumps(self) -> None:
        hook = self.run_watch({1: outcomes.MERGED, 2: outcomes.HANDED_BACK, 3: outcomes.MERGED})
        self.assertEqual(hook.landed, [1, 3])
        self.assertGreaterEqual(hook.pumps, 3)

    def test_a_watch_without_the_hook_still_runs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, [[fakes.pr(1)]], {1: outcomes.MERGED})
            results, _, _ = loop.run(world.fx, fakes.opts(idle=1, poll=30))
        self.assertEqual(results[0][1], outcomes.MERGED)
