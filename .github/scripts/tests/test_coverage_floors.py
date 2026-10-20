#!/usr/bin/env python3
"""A coverage floor drops only with an inline reason (#963).

The unit cases pin the rule on literal baselines; `ThisBranch` runs it over
the real diff against `origin/main`, which is how the check holds in
`make gates` and lint.yml.
"""

from __future__ import annotations

import os
import subprocess
import unittest

from coverage_floors import REPO_ROOT, against, findings

BASE = "# floors\napp 95.00\napi 86.00\n"


def check(head: str, present: tuple[str, ...] = ("app", "api")) -> list[str]:
    return findings(BASE, head, lambda m: m in present)


class Rule(unittest.TestCase):
    def test_a_silent_drop_is_flagged(self):
        found = check("app 95.00\napi 85.00\n")
        self.assertEqual(len(found), 1)
        self.assertIn("`api` floor dropped 86.00 -> 85.00", found[0])

    def test_a_drop_with_a_reason_passes(self):
        self.assertEqual(check("app 95.00\napi 85.00  # reason: Sound moved out (#1)\n"), [])

    def test_an_empty_reason_is_still_silent(self):
        self.assertEqual(len(check("app 95.00\napi 85.00  # reason:\n")), 1)

    def test_a_plain_comment_is_not_a_reason(self):
        self.assertEqual(len(check("app 95.00\napi 85.00  # lowered\n")), 1)

    def test_a_raise_and_a_new_floor_pass(self):
        self.assertEqual(check("app 95.50\napi 86.00\nphysics 97.00\n"), [])

    def test_a_removed_floor_on_a_live_module_is_flagged(self):
        self.assertIn("`api` floor (86.00) was removed", check("app 95.00\n")[0])

    def test_a_removed_floor_on_a_gone_module_passes(self):
        self.assertEqual(check("app 95.00\n", present=("app",)), [])


class ThisBranch(unittest.TestCase):
    def test_no_floor_is_lowered_silently_against_main(self):
        known = subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", "origin/main"],
            cwd=REPO_ROOT,
            capture_output=True,
        )
        if known.returncode != 0:
            if os.environ.get("CI"):
                self.fail("origin/main is not fetched; the lint job checks out fetch-depth: 0")
            self.skipTest("origin/main is not fetched in this clone")
        self.assertEqual(against("origin/main", REPO_ROOT), [])


if __name__ == "__main__":
    unittest.main()
