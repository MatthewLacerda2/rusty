#!/usr/bin/env python3
"""A hand ask too large to measure is refused before a shard starts (#2371).

Past `HEADROOM` times what `MAX_SHARDS` runners measure, a whole-file ask ends
red in the `plan` job with each file's count, largest first; a `diff` ask past
the cap is never refused, since no narrower honest ask exists for it.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tests import SCRIPTS, load

request = load("mutants-request.py")
BUDGET = 9900
LIMIT = int(request.HEADROOM * request.fits(BUDGET))


def plan(engine: int, lint: int = 0, kind: str = "paths") -> dict:
    """A plan as `mutants-scope.py check` leaves it, each part's counts by file."""
    root = {"crates/a/src/big.rs": engine - engine // 3, "crates/a/src/small.rs": engine // 3}
    return {"kind": kind, "parts": [{"root": ".", "slug": "root", "by_file": root},
                                    {"root": "tools/lint", "slug": "tools-lint",
                                     "by_file": {"tools/lint/src/rules/x.rs": lint} if lint else {}}]}


class Refusal(unittest.TestCase):
    def test_the_threshold_is_headroom_over_what_fits(self) -> None:
        fit = request.fits(BUDGET)
        self.assertEqual(fit, int(request.MAX_SHARDS * (BUDGET / 60 - request.SETUP_MINUTES)
                                  / request.MINUTES_PER_MUTANT))
        self.assertEqual(request.shards(fit, BUDGET)[0], request.MAX_SHARDS, "what fits takes every runner")
        self.assertEqual(request.HEADROOM, 1.5)

    def test_under_and_at_the_threshold_it_runs(self) -> None:
        self.assertIsNone(request.refusal(plan(10), BUDGET))
        self.assertIsNone(request.refusal(plan(LIMIT), BUDGET))

    def test_over_the_threshold_it_is_refused_with_its_files_largest_first(self) -> None:
        why = request.refusal(plan(LIMIT, lint=1), BUDGET)
        self.assertIsNotNone(why)
        self.assertIn(f"plans {LIMIT + 1} mutants", why)
        self.assertIn(f"measures about {request.fits(BUDGET)}", why)
        big, small, lint = (why.index(f"`{f}`") for f in
                            ("crates/a/src/big.rs", "crates/a/src/small.rs", "tools/lint/src/rules/x.rs"))
        self.assertLess(big, small)
        self.assertLess(small, lint, "a standalone part's file is named from the repository root")

    def test_a_long_listing_sums_the_tail(self) -> None:
        many = {"kind": "paths", "parts": [{"by_file": {f"crates/a/src/f{i:02}.rs": 10 for i in range(40)}}]}
        why = request.refusal(many, BUDGET)
        self.assertEqual(why.count("\n- `"), request.LISTED)
        self.assertIn(f"- {40 - request.LISTED} more files: {(40 - request.LISTED) * 10}", why)

    def test_a_diff_over_the_cap_is_not_refused(self) -> None:
        self.assertIsNone(request.refusal(plan(10 * LIMIT, kind="diff"), BUDGET))

    def test_the_command_exits_red_and_says_why(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            here = Path(d)
            (here / "plan.json").write_text(json.dumps(plan(LIMIT + 5)))
            run = [sys.executable, str(SCRIPTS / "mutants-request.py"), "refuse", str(here / "plan.json"),
                   "--budget-seconds", str(BUDGET)]
            done = subprocess.run(run, capture_output=True, text=True)
            self.assertEqual(done.returncode, 1)
            self.assertIn("No shard started", done.stdout)
            (here / "plan.json").write_text(json.dumps(plan(0)))
            self.assertEqual(subprocess.run(run, capture_output=True).returncode, 0, "nothing planned fits")

    def test_the_workflow_refuses_in_plan_before_sizing(self) -> None:
        text = (SCRIPTS.parent / "workflows" / "mutants-on-request.yml").read_text()
        self.assertLess(text.index("mutants-request.py refuse"), text.index("mutants-request.py shards"))
        self.assertIn('[ "$FITS" = failure ]', text)


if __name__ == "__main__":
    unittest.main()
