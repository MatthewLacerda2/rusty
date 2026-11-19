"""The slow-test signal's baseline and pace (#2350): `main`'s longest time over
its last runs is what a test is held to, and a shard whose runner is slow raises
the bar by its own pace, so a re-run of an unchanged tree on a slow runner names
nothing while one test that grew alone is still named."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import slowest_tests
from slowest_tests import Case, pace, signal

#: Twelve heavy tests `main` ran at 10 s, enough to read a pace from.
HEAVY = {f"rusty::main gpu::heavy_{i}": 10.0 for i in range(12)}


def shard(factor: float, **extra: float) -> list[Case]:
    """The heavy tests at `factor` times `main`, plus `extra` tests by name."""
    found = [Case("rusty::main", f"gpu::heavy_{i}", 10.0 * factor) for i in range(12)]
    return found + [Case("rusty::main", n, s) for n, s in extra.items()]


def new(lines: list[str]) -> list[str]:
    return [l for l in lines if l.startswith("new over") and "none" not in l]


class Pace(unittest.TestCase):
    def test_a_shard_at_mains_pace_reads_one(self):
        self.assertEqual(pace(shard(0.8), HEAVY), 1.0)

    def test_a_slow_runner_reads_its_slowdown(self):
        self.assertAlmostEqual(pace(shard(2.0), HEAVY), 2.0)

    def test_too_few_heavy_tests_read_one(self):
        self.assertEqual(pace(shard(2.0)[:slowest_tests.PACE_MIN - 1], HEAVY), 1.0)

    def test_on_a_slow_runner_an_unchanged_test_is_near_not_new(self):
        lines = signal(shard(2.0, **{"gpu::edge": 21.6}), {**HEAVY, "rusty::main gpu::edge": 8.5})
        self.assertEqual(new(lines), [])
        self.assertIn("near over 20 s: rusty::main gpu::edge 21.6 s (main: 8.5 s)", lines)
        self.assertFalse(any(l.startswith("::warning") for l in lines))

    def test_at_mains_pace_one_test_that_grew_alone_is_new(self):
        lines = signal(shard(1.0, **{"gpu::grew": 30.0}), {**HEAVY, "rusty::main gpu::grew": 10.0})
        self.assertEqual(new(lines), ["new over 20 s: rusty::main gpu::grew 30.0 s (main: 10.0 s)"])

    def test_the_pace_is_printed(self):
        lines = signal(shard(2.0), HEAVY)
        self.assertIn("pace: 2.00 times main's longest", next(l for l in lines if l.startswith("pace:")))


class Baseline(unittest.TestCase):
    def test_a_test_is_held_to_its_longest_time_over_mains_runs(self):
        with tempfile.TemporaryDirectory() as tmp:
            for run, seconds in (("1", 9.0), ("2", 17.0), ("3", 12.0)):
                (Path(tmp) / run).mkdir()
                (Path(tmp) / run / "junit-dev.xml").write_text(
                    f'<testsuites><testcase classname="rusty::main" name="gpu::t" time="{seconds}"/></testsuites>')
            self.assertEqual(slowest_tests.baseline([tmp]), {"rusty::main gpu::t": 17.0})


if __name__ == "__main__":
    unittest.main()
