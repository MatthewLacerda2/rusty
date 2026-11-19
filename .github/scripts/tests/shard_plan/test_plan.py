"""The plan's one promise (#2397): every test, known to `main` or added since,
runs in exactly one shard, whatever the times said; and the times decide the
balance. The filters are read by `filterset.matches`, not by nextest; the PR
that added this checked the same plan against `cargo nextest list` too."""

from __future__ import annotations

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

import shard_plan
from tests.shard_plan.filterset import matches

#: A suite like the macOS leg's: two binaries, a long tail of light tests and a
#: few heavy GPU ones that all sort together, as `count:` dealt them.
TIMES = {**{f"rusty::integration gpu::test_games::game_{i}": 30.0 + i for i in range(8)},
         **{f"rusty::integration logic::case_{i}_{w}": 0.1 * (i % 7) for i in range(300) for w in ("ab", "q")},
         **{f"rusty-render gpu_draw_{i}": 2.0 for i in range(40)}}
#: Tests `main` never ran: new names, a one-letter name, a binary it never had.
NEW = [("rusty::integration", "logic::brand_new"), ("rusty-render", "x"), ("rusty-fresh", "gpu_first")]


def everyone() -> list[tuple[str, str]]:
    return [tuple(k.split(" ", 1)) for k in TIMES] + NEW


def owners(found: dict[str, str], binary: str, name: str) -> list[str]:
    return [shard for shard, text in found.items() if matches(text, binary, name)]


class Coverage(unittest.TestCase):
    def test_every_test_runs_in_exactly_one_shard(self):
        for shards in (1, 2, 3):
            found, _ = shard_plan.plan(TIMES, shards)
            self.assertEqual(sorted(found), [f"{k}/{shards}" for k in range(1, shards + 1)])
            for binary, name in everyone():
                with self.subTest(shards=shards, test=name):
                    self.assertEqual(len(owners(found, binary, name)), 1)

    def test_a_shard_that_is_not_the_complement_leaves_a_test_out(self):
        # The flag case: shard 2/2 written as its own selection, not as "every
        # test 1/2 did not take", loses a test `main` never ran.
        dealt = shard_plan.deal(shard_plan.units(TIMES), 2)
        heavy = shard_plan.exact([u[2] for d in dealt for u in d if u[1] == "test"])
        broken = {"1/2": shard_plan.selection(dealt[0], heavy), "2/2": shard_plan.selection(dealt[1], heavy)}
        lost = [t for t in everyone() if not owners(broken, *t)]
        self.assertIn(("rusty-render", "x"), lost)

    def test_the_filters_take_every_test_once_whatever_the_selections(self):
        for picked in (["all()"], ["none()"], ["test(=a)", "all()"], ["all()", "none()", "test(/a$/)"]):
            made = dict(enumerate(shard_plan.filters(picked)))
            for name in ("a", "b", "ba"):
                with self.subTest(picked=picked, name=name):
                    self.assertEqual(len(owners(made, "bin", name)), 1)


class Balance(unittest.TestCase):
    def test_the_heavy_tests_are_spread_and_the_loads_even(self):
        found, loads = shard_plan.plan(TIMES, 2)
        heavy = [k for k in TIMES if "test_games" in k]
        first = [k for k in heavy if owners(found, *k.split(" ", 1)) == ["1/2"]]
        self.assertEqual(len(first), len(heavy) // 2)
        self.assertLess(abs(loads[0] - loads[1]), 0.02 * sum(loads))

    def test_the_plan_is_the_same_on_every_call(self):
        self.assertEqual(shard_plan.plan(TIMES, 2), shard_plan.plan(dict(reversed(TIMES.items())), 2))

    def test_the_filter_stays_one_argument_with_long_names(self):
        long = {f"rusty::integration gpu::{'a' * 180}_{i}": 10.0 for i in range(500)}
        found, _ = shard_plan.plan(long, 2)
        self.assertLess(max(len(f.encode()) for f in found.values()), 128 * 1024)


class Args(unittest.TestCase):
    def test_a_plan_gives_its_filter(self):
        found, _ = shard_plan.plan(TIMES, 2)
        self.assertEqual(shard_plan.args(json.dumps(found), "2/2")[0], ["-E", found["2/2"]])

    def test_no_plan_or_another_matrix_splits_by_count_on_every_shard(self):
        two = json.dumps(shard_plan.plan(TIMES, 2)[0])
        for raw, why in (("", "no plan"), ("{}", "no plan"), ("not json", "not JSON"),
                         ("5", "not a mapping"), ('["1/3"]', "not a mapping"), ("null", "not a mapping"),
                         ('{"1/3": "", "2/3": "x", "3/3": "y"}', "where the matrix"),
                         (two, "where the matrix has"), ('{"1/3": 1, "2/3": "x", "3/3": "y"}', "where the matrix")):
            for shard in ("1/3", "2/3", "3/3"):
                with self.subTest(raw=raw[:20], shard=shard):
                    found, line = shard_plan.args(raw, shard)
                    self.assertEqual(found, ["--partition", f"count:{shard}"])
                    self.assertIn(why, line)

    def test_no_report_plans_nothing(self):
        with tempfile.TemporaryDirectory() as empty:
            self.assertEqual(run_plan(str(Path(empty) / "absent")), ("{}", "plan: none"))

    def test_a_report_with_no_timed_test_plans_nothing(self):
        # The flag case: a plan from no times would give 1/2 `none()`, a shard of
        # no test, which nextest fails.
        self.assertEqual(shard_plan.plan({}, 2), ({}, [0.0, 0.0]))
        self.assertEqual(shard_plan.plan({"rusty-render gpu_x": 3.0}, 2)[0], {})
        with tempfile.TemporaryDirectory() as base:
            (Path(base) / "junit.xml").write_text('<testsuites><testsuite name="rusty"/></testsuites>')
            self.assertEqual(run_plan(base), ("{}", "plan: none"))

    def test_a_plan_gives_every_shard_a_test(self):
        found, _ = shard_plan.plan(TIMES, 3)
        for shard, text in found.items():
            with self.subTest(shard=shard):
                self.assertTrue(any(matches(text, *t) for t in everyone()))


def run_plan(base: str) -> tuple[str, str]:
    """`shard_plan.py plan <base> --shards 2`'s plan and its stderr's first word pair."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        assert shard_plan.main(["plan", base, "--shards", "2"]) == 0
    return out.getvalue().strip(), " ".join(err.getvalue().split()[:2]).rstrip(",")


if __name__ == "__main__":
    unittest.main()
