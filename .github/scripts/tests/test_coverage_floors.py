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

from coverage_floors import REPO_ROOT, GitError, against, findings

BASE = "# floors\napp 95.00\napi 86.00\n"


def check(head: str, present: tuple[str, ...] = ("app", "api")) -> list[str]:
    return findings(BASE, head, lambda m: f"src/{m}" if m in present else None,
                    lambda m: ["a", "b"])


def split(base: str, head: str, folders: list[str]) -> list[str]:
    """`findings` for a crate `base` floors whole and `head` floors by folder."""
    return findings(base, head, lambda m: f"crates/{m}/src", lambda m: folders)


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

    def test_a_drop_keeping_the_old_reason_is_flagged(self):
        base = "api 85.00  # reason: X\n"
        found = findings(base, "api 70.00  # reason: X\n", lambda m: f"src/{m}")
        self.assertIn("`api` floor dropped 85.00 -> 70.00", found[0])

    def test_the_floor_is_printed_as_written(self):
        self.assertIn("86.00 -> 85.999", check("app 95.00\napi 85.999\n")[0])

    def test_a_raise_and_a_new_floor_pass(self):
        self.assertEqual(check("app 95.50\napi 86.00\nphysics 97.00\n"), [])

    def test_a_removed_floor_on_a_live_module_is_flagged(self):
        found = check("app 95.00\n")
        self.assertIn("`api` floor (86.00) was removed while src/api/ still exists", found[0])

    def test_a_floor_split_into_halves_at_least_as_high_passes(self):
        self.assertEqual(check("app 95.00\napi/a 90.00\napi/b 86.00\n"), [])

    def test_one_low_half_standing_in_for_a_whole_crate_is_flagged(self):
        found = split("script 88.00\n", "script/api 0.00\n", ["api", "scripting"])
        self.assertEqual(len(found), 2)
        self.assertIn("split into `script/api` at 0.00, below it", found[0])
        self.assertIn("crates/script/src/scripting/ has no `script/scripting` floor", found[1])

    def test_a_low_half_of_one_folder_is_flagged(self):
        found = split("physics 97.00\n", "physics/shape 1.00\n", ["shape"])
        self.assertEqual(len(found), 1)
        self.assertIn("split into `physics/shape` at 1.00", found[0])

    def test_a_half_below_the_old_floor_passes_with_a_reason(self):
        head = "script/api 86.00  # reason: measured alone\nscript/scripting 96.00\n"
        self.assertEqual(split("script 88.00\n", head, ["api", "scripting"]), [])

    def test_a_removed_floor_on_a_gone_module_passes(self):
        self.assertEqual(check("app 95.00\n", present=("app",)), [])


class ThisBranch(unittest.TestCase):
    def test_a_failing_git_names_its_error(self):
        with self.assertRaisesRegex(GitError, "no-such-ref-963"):
            against("no-such-ref-963", REPO_ROOT)

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
