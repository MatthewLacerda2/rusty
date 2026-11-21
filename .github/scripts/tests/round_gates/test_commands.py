"""The commands a round runs, the recipes they come from, and the line it ends on."""

from __future__ import annotations

import re
import unittest

import round_gates

from .fixtures import CI, MAKEFILE, RECIPES, plan


class Recipes(unittest.TestCase):
    def test_the_suite_seed_and_hint_are_the_makefiles_and_cis(self):
        suite = re.search(r"^DEFAULT_SUITE := -E '(.*)'$", MAKEFILE, re.M)[1]
        self.assertEqual(RECIPES.suite, suite)
        self.assertIn(f"'{RECIPES.seed}'", CI)
        self.assertIn(RECIPES.hint, MAKEFILE)
        self.assertIn("cargo install --locked cargo-nextest", RECIPES.hint)

    def test_tools_lints_clippy_is_the_makefiles_line(self):
        self.assertEqual(RECIPES.lint_clippy[:2], ["cargo", "clippy"])
        self.assertIn(" ".join(RECIPES.lint_clippy), MAKEFILE)


class Commands(unittest.TestCase):
    def test_make_tests_four_runs_on_the_workspaces_feature_sets(self):
        default, dev, doc, doc_dev = round_gates.test_commands(["rusty-physics", "rusty-time"], RECIPES.suite)
        picked = "package(=rusty-physics) | package(=rusty-time)"
        self.assertEqual(default[-2:], ["-E", f"({RECIPES.suite}) & ({picked})"])
        self.assertEqual(dev[-4:], ["--features", "dev", "-E", picked])
        self.assertEqual(doc, ["cargo", "test", "--doc", "--locked"])
        self.assertEqual(doc_dev, [*doc, "--features", "dev"])

    def test_no_run_selects_by_package_so_no_crate_builds_alone(self):
        # A `-p` build loses the features the rest of the workspace unifies in.
        for crate in ("rusty-time", "rusty-physics", "rusty"):
            for cmd in round_gates.test_commands([crate], RECIPES.suite):
                with self.subTest(crate=crate, cmd=cmd):
                    self.assertNotIn("-p", cmd)
                    self.assertFalse(any(f.startswith(crate + "/") for f in cmd))

    def test_plan_prints_test_runs_with_the_seed(self):
        nextest = round_gates.test_commands(["rusty-time"], RECIPES.suite)[0]
        self.assertTrue(round_gates.shown(nextest, "1784").startswith("PROPTEST_RNG_SEED=1784 cargo nextest"))
        self.assertIn("'(not group(gpu*)) & (package(=rusty-time))'", round_gates.shown(nextest, "1"))
        self.assertEqual(round_gates.shown(["make", "clippy"], "1784"), "make clippy")


class Summary(unittest.TestCase):
    def test_it_names_what_ran_and_what_ci_still_runs(self):
        line = round_gates.summary(plan("crates/time/src/clock.rs"), "abc1234", 4)
        self.assertTrue(line.startswith("Round gates on abc1234: ran quick-gates; clippy; doc; tests of rusty-time"))
        self.assertIn("package() filter", line)
        self.assertIn("Left to CI: the tests of the other 3 workspace packages, deny.", line)
        self.assertIn("Skipped by CI too: fuzz-check.", line)

    def test_a_diff_ci_does_not_build_leaves_nothing_to_it(self):
        line = round_gates.summary(plan("docs/testing.md"), "abc", 4)
        self.assertIn("Left to CI: nothing.", line)
        self.assertIn("Skipped by CI too: clippy, doc, the tests of all 4 workspace packages, deny, fuzz-check.", line)

    def test_a_fuzz_change_runs_fuzz_check_and_says_nothing_of_skipping_it(self):
        line = round_gates.summary(plan("crates/core/src/lib.rs"), "abc", 4)
        self.assertIn("fuzz-check", line.split("Left to CI")[0])
        self.assertNotIn("Skipped by CI too", line)


if __name__ == "__main__":
    unittest.main()
