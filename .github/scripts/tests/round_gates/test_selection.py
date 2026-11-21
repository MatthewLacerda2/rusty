"""Which gates a round's paths select, and which paths fall back to `make gates`."""

from __future__ import annotations

import unittest

import round_gates

from .fixtures import plan, steps

QUICK = [("quick-gates", ["make", "--no-print-directory", "quick-gates"])]
GATES = [("gates", ["make", "--no-print-directory", "gates"])]


class Selection(unittest.TestCase):
    def test_a_changed_file_runs_its_crates_tests(self):
        p = plan("crates/physics/src/shape/build.rs")
        self.assertIsNone(p.fallback)
        self.assertEqual(p.crates, ["rusty-physics"])
        self.assertTrue(p.clippy and p.doc and p.code)

    def test_a_leaf_crate_change_skips_the_root_tests(self):
        p = plan("crates/time/src/lib.rs", "crates/time/src/clock.rs")
        self.assertEqual(p.crates, ["rusty-time"])

    def test_the_root_tests_run_for_tests_or_the_root_crate(self):
        self.assertEqual(plan("tests/physics_api.rs").crates, ["rusty"])
        self.assertEqual(plan("src/bin/play.rs").crates, ["rusty"])

    def test_a_docs_only_diff_runs_quick_gates_alone(self):
        p = plan("docs/testing.md", "crates/render/src/CLAUDE.md", "README.md")
        self.assertEqual(round_gates.steps(p, None), QUICK)
        self.assertFalse(p.code)

    def test_tooling_outside_rust_runs_quick_gates_alone(self):
        self.assertEqual(steps(".github/scripts/coder.py", ".claude/skills/x/SKILL.md"), QUICK)

    def test_a_tools_lint_change_runs_its_clippy_not_the_workspaces(self):
        p = plan("tools/lint/src/main.rs")
        self.assertTrue(p.lint_clippy)
        self.assertEqual((p.clippy, p.crates, p.doc), (False, [], False))
        names = [name for name, _ in steps("tools/lint/src/main.rs")]
        self.assertEqual(names, ["quick-gates", "clippy (tools/lint)"])
        self.assertIn("tools/lint/Cargo.toml", steps("tools/lint/src/main.rs")[1][1])

    def test_a_workspace_change_beside_tools_lint_runs_make_clippy_once(self):
        names = [name for name, _ in steps("tools/lint/src/main.rs", "crates/time/src/clock.rs")]
        self.assertIn("clippy", names)
        self.assertNotIn("clippy (tools/lint)", names)

    def test_markdown_code_reads_inside_a_crate_runs_that_crate(self):
        p = plan("crates/core/src/project/game_claude.md")
        self.assertEqual((p.fallback, p.crates, p.clippy, p.code), (None, ["rusty-core"], False, True))

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
                self.assertEqual(round_gates.steps(p, None), GATES)
                self.assertIn("make gates in full", round_gates.summary(p, "abc", 4))

    def test_a_fixture_falls_back_wherever_it_lives(self):
        for path in ("tests/fixtures/scene.json", "crates/physics/tests/fixtures/box.gltf",
                     "crates/core/src/fixtures/a.ron", "fixtures/x.bin"):
            with self.subTest(path=path):
                self.assertIn("fixtures are read by tests", plan(path).fallback)

    def test_a_module_named_build_is_not_a_build_script(self):
        self.assertIsNone(plan("crates/physics/src/build.rs").fallback)


if __name__ == "__main__":
    unittest.main()
