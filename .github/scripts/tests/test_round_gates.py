#!/usr/bin/env python3
"""#2452: `make round-gates` runs the gates a round's files can break, and falls
back to `make gates` for a change it cannot place."""

from __future__ import annotations

import unittest

import round_gates

CI = (round_gates.ROOT / round_gates.CI).read_text()
MEMBERS = {"": "rusty", "crates/physics": "rusty-physics", "crates/core": "rusty-core",
           "crates/time": "rusty-time"}
FEATURES = {"rusty": ["default", "dev", "editor"], "rusty-physics": ["dev", "editor"],
            "rusty-core": ["dev", "editor"], "rusty-time": []}


def plan(*paths: str) -> round_gates.Plan:
    return round_gates.plan(list(paths), MEMBERS, CI)


class Selection(unittest.TestCase):
    def test_a_changed_file_runs_its_crates_tests(self):
        p = plan("crates/physics/src/shape/build.rs")
        self.assertIsNone(p.fallback)
        self.assertEqual(p.crates, ["rusty-physics"])
        self.assertTrue(p.clippy and p.doc)

    def test_a_leaf_crate_change_skips_the_root_tests(self):
        p = plan("crates/time/src/lib.rs", "crates/time/src/clock.rs")
        self.assertEqual(p.crates, ["rusty-time"])
        self.assertNotIn("rusty", p.crates)

    def test_the_root_tests_run_for_tests_or_the_root_crate(self):
        self.assertEqual(plan("tests/physics_api.rs").crates, ["rusty"])
        self.assertEqual(plan("src/bin/play.rs").crates, ["rusty"])

    def test_a_docs_only_diff_runs_quick_gates_alone(self):
        p = plan("docs/testing.md", "crates/render/src/CLAUDE.md", "README.md")
        self.assertEqual(round_gates.steps(p, FEATURES), [("quick-gates", ["make", "--no-print-directory", "quick-gates"])])
        self.assertIn("Left to CI: clippy, doc, the tests of all 4", round_gates.summary(p, "abc", 4))

    def test_tooling_runs_quick_gates_and_rust_tooling_adds_clippy(self):
        self.assertEqual(round_gates.steps(plan(".github/scripts/coder.py", ".claude/skills/x/SKILL.md"),
                                           FEATURES)[0][0], "quick-gates")
        self.assertEqual(len(round_gates.steps(plan(".github/scripts/coder.py"), FEATURES)), 1)
        p = plan("tools/lint/src/main.rs")
        self.assertTrue(p.clippy)
        self.assertEqual((p.crates, p.doc), ([], False))

    def test_markdown_code_reads_inside_a_crate_runs_that_crate(self):
        p = plan("crates/core/src/project/game_claude.md")
        self.assertEqual((p.fallback, p.crates, p.clippy), (None, ["rusty-core"], False))

    def test_fuzz_and_deny_follow_what_they_read(self):
        self.assertTrue(plan("crates/core/src/lib.rs").fuzz)
        self.assertFalse(plan("crates/time/src/clock.rs").fuzz)
        p = plan("deny.toml")
        self.assertTrue(p.deny and not p.clippy)


class Fallback(unittest.TestCase):
    def test_unplaceable_changes_run_everything(self):
        for path in ("Makefile", ".github/workflows/ci.yml", "Cargo.lock", "Cargo.toml",
                     "crates/core/Cargo.toml", "build.rs", "crates/core/build.rs",
                     ".config/nextest.toml", "rust-toolchain.toml", "docs/api/Physics.md",
                     "engine/shaders/pbr.wgsl", "games/arena/main.lua"):
            with self.subTest(path=path):
                p = plan("crates/time/src/clock.rs", path)
                self.assertIn(path, p.fallback)
                self.assertEqual(round_gates.steps(p, FEATURES), [("gates", ["make", "--no-print-directory", "gates"])])
                self.assertIn("make gates in full", round_gates.summary(p, "abc", 4))

    def test_a_module_named_build_is_not_a_build_script(self):
        self.assertIsNone(plan("crates/physics/src/build.rs").fallback)


class Commands(unittest.TestCase):
    def test_make_tests_four_runs_on_each_crates_own_features(self):
        default, dev, doc, doc_dev = round_gates.test_commands(["rusty-physics", "rusty-time"], FEATURES)
        self.assertEqual(default[-4:], ["--features", "rusty-physics/editor", "-E", "not group(gpu*)"])
        self.assertIn("-p", default)
        self.assertEqual(dev[-2:], ["--features", "rusty-physics/editor,rusty-physics/dev"])
        self.assertEqual(doc[:4], ["cargo", "test", "--doc", "--locked"])
        self.assertEqual(doc_dev[-1], "rusty-physics/editor,rusty-physics/dev")

    def test_a_crate_without_features_gets_no_features_flag(self):
        default, dev, _, _ = round_gates.test_commands(["rusty-time"], FEATURES)
        self.assertNotIn("--features", dev)
        self.assertEqual(default[-2:], ["-E", "not group(gpu*)"])

    def test_the_summary_names_what_ran_and_what_it_left(self):
        line = round_gates.summary(plan("crates/time/src/clock.rs"), "abc1234", 4)
        self.assertTrue(line.startswith("Round gates on abc1234: ran quick-gates; clippy; doc; tests of rusty-time"))
        self.assertIn("Left to CI: the tests of the other 3 workspace packages, fuzz-check, deny.", line)


if __name__ == "__main__":
    unittest.main()
