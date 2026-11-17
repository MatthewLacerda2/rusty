#!/usr/bin/env python3
"""An on-request run fits its plan and still answers once (#2283).

The plan gets runners enough for its share to end inside the budget, never
more than the cap; each package's shard is turned so the plan is dealt as one
list; and the shards' outcomes merge into the one-run layout the report reads,
a lost shard named, never silently clean.
"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tests import SCRIPTS, load

request = load("mutants-request.py")
WORKFLOW = SCRIPTS.parent / "workflows" / "mutants-on-request.yml"
report = load("mutants-report.py")


def outcome(name: str, summary: str, diff: str | None = None) -> dict:
    o = {"scenario": {"Mutant": {"name": name}}, "summary": summary}
    return {**o, "diff_path": diff} if diff else o


def shard(root: Path, k: int, package: str, status: str, outcomes: list[dict]) -> Path:
    out = root / "shards" / f"mutants-shard-{k}" / "mutants-root" / package
    (out / "mutants.out" / "diff").mkdir(parents=True)
    (out / "status").write_text(status + "\n")
    (out / "mutants.out" / "outcomes.json").write_text(json.dumps({"outcomes": outcomes}))
    return out


PLAN = {"parts": [{"root": ".", "slug": "root", "planned": 4}]}


class Shards(unittest.TestCase):
    def test_a_small_plan_takes_one_runner(self) -> None:
        self.assertEqual(request.shards(10, 9900, minutes=3)[0], 1)
        self.assertEqual(request.shards(0, 9900)[0], 1)

    def test_a_large_plan_is_spread_and_capped(self) -> None:
        usable = 9900 / 60 - request.SETUP_MINUTES
        self.assertEqual(request.shards(int(usable // 3) + 1, 9900, minutes=3)[0], 2)
        n, why = request.shards(10_000, 9900, minutes=3)
        self.assertEqual(n, request.MAX_SHARDS)
        self.assertIn("capped", why)

    def test_a_standalone_part_counts_toward_the_size(self) -> None:
        usable = 9900 / 60 - request.SETUP_MINUTES
        engine = int(usable // 3)
        self.assertEqual(request.shards(engine, 9900, minutes=3)[0], 1)
        self.assertEqual(request.shards(engine, 9900, minutes=3, standalone=5)[0], 2)

    def test_the_report_downloads_every_possible_shard_by_name(self) -> None:
        """`download-artifact` unpacks a lone `pattern` match flat, losing a one-shard run's shard."""
        text = WORKFLOW.read_text()
        self.assertNotIn("pattern: mutants-shard-", text)
        for k in range(request.MAX_SHARDS):
            self.assertIn(f"name: mutants-shard-{k}\n          path: run/shards/mutants-shard-{k}\n", text)

    def test_the_tests_a_nextest_list_selects(self) -> None:
        case = lambda status, ignored=False: {"filter-match": {"status": status}, "ignored": ignored}
        listed = {"rust-suites": {"a": {"testcases": {"x": case("matches"), "y": case("mismatch"),
                                                      "z": case("matches", True)}}}}
        self.assertEqual(request.tests(listed), 1)
        self.assertEqual(request.tests({"rust-suites": {}}), 0)


class Packages(unittest.TestCase):
    def test_each_package_with_the_mutants_before_it(self) -> None:
        listed = [{"package": "b"}, {"package": "a"}, {"package": "b"}, {"package": "c"}]
        self.assertEqual(request.packages(listed), [("a", 0), ("b", 1), ("c", 3)])

    def test_the_offset_deals_the_plan_round_robin_as_one_list(self) -> None:
        """The workflow's `(K - offset) mod N`, with cargo-mutants' `i mod N`."""
        sizes, n, got = {"a": 1, "b": 1, "c": 1, "d": 2}, 4, []
        offsets = dict(request.packages([{"package": p} for p, c in sizes.items() for _ in range(c)]))
        for k in range(n):
            mine = sum(1 for p, c in sizes.items() for i in range(c) if i % n == (k - offsets[p]) % n)
            got.append(mine)
        self.assertEqual(sorted(got), [1, 1, 1, 2])

    def test_the_priority_names_its_package_exactly(self) -> None:
        self.assertIn("filter = 'package(=rusty-shell)'", request.priority("rusty-shell"))
        self.assertIn("priority = 100", request.priority("rusty-shell"))


class Merge(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = Path(tempfile.mkdtemp())

    def merged(self, total: int) -> tuple[list[str], dict, str]:
        lost = request.merge(PLAN, self.dir, self.dir / "shards", total)
        run = json.loads((self.dir / "mutants-root" / "mutants.out" / "outcomes.json").read_text())
        return lost, run, (self.dir / "status-root").read_text().strip()

    def unmeasured(self) -> list[str]:
        return (self.dir / "lost-root.txt").read_text().splitlines()

    def test_every_shard_and_package_becomes_one_run(self) -> None:
        out = shard(self.dir, 0, "a", "2", [outcome("a.rs:1: x", "MissedMutant", "diff/x.diff")])
        (out / "mutants.out" / "diff" / "x.diff").write_text("--- a.rs\n+++ x\n")
        shard(self.dir, 0, "b", "0", [outcome("b.rs:1: y", "CaughtMutant")])
        shard(self.dir, 1, "a", "0", [outcome("a.rs:2: z", "CaughtMutant")])
        lost, run, status = self.merged(2)
        self.assertEqual((lost, status, len(run["outcomes"])), ([], "2", 3))
        found = report.diffs(self.dir / "mutants-root" / "mutants.out", run)
        self.assertIn("+++ x", found)

    def test_a_budget_stop_in_any_shard_stops_the_part(self) -> None:
        shard(self.dir, 0, "a", "2", [])
        shard(self.dir, 1, "a", "124", [])
        self.assertEqual(self.merged(2)[2], "124")

    def test_a_broken_package_is_named_and_the_others_still_count(self) -> None:
        shard(self.dir, 0, "a", "2", [outcome("a.rs:1: x", "MissedMutant")])
        shard(self.dir, 1, "b", "4", [outcome("b.rs:1: y", "CaughtMutant")])
        _, run, status = self.merged(2)
        self.assertEqual((status, len(run["outcomes"])), ("2", 2))
        self.assertEqual(self.unmeasured(), ["`b` in shard 1: cargo-mutants exited 4; its mutants are unmeasured"])
        text, _, broken = report.report(PLAN, "Scoped.", self.dir)
        self.assertIn("a.rs:1: x", text)
        self.assertIn("2 of 4 planned mutants measured, 2 never reached", text)
        self.assertTrue(broken)

    def test_a_shard_that_uploaded_nothing_is_lost_and_the_report_says_so(self) -> None:
        shard(self.dir, 0, "a", "0", [outcome("a.rs:1: x", "CaughtMutant")])
        lost, _, status = self.merged(2)
        self.assertEqual((lost, status), (["root 1"], "0"))
        (self.dir / "lost-root.txt").write_text("shard 1: the mutation job was torn down\n")
        text, _, broken = report.report(PLAN, "Scoped.", self.dir)
        self.assertIn("Not every shard or package reported: 1 of 4 planned mutants measured, 3 never reached", text)
        self.assertIn("Unmeasured: shard 1: the mutation job was torn down", text)
        self.assertNotIn("✅", text)
        self.assertTrue(broken)

    def test_every_shard_lost_never_blames_the_budget(self) -> None:
        lost, _, status = self.merged(2)
        self.assertEqual((lost, status), (["root 0", "root 1"], "0"))
        (self.dir / "lost-root.txt").write_text("shard 0 left no report: x\nshard 1 left no report: y\n")
        text, _, broken = report.report(PLAN, "Scoped.", self.dir)
        self.assertNotIn("budget", text)
        self.assertIn("Nothing was measured: not a result.", text)
        self.assertEqual(len(broken), 2)

    def test_a_shard_with_no_status_for_the_part_is_broken(self) -> None:
        (self.dir / "shards" / "mutants-shard-0").mkdir(parents=True)
        self.merged(1)
        self.assertIn("left no status", self.unmeasured()[0])

    def test_a_failed_baseline_reads_as_broken(self) -> None:
        top = self.dir / "shards" / "mutants-shard-0"
        top.mkdir(parents=True)
        (top / "status-root").write_text("after the unmutated tests failed (the Baseline step)\n")
        self.merged(1)
        _, _, broken = report.report(PLAN, "Scoped.", self.dir)
        self.assertTrue(any("Baseline" in b for b in broken))


if __name__ == "__main__":
    unittest.main()
