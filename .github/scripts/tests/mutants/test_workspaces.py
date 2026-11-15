#!/usr/bin/env python3
"""A scope reaches every cargo workspace it names, as one plan each (#2201).

`tools/lint` opens its own `[workspace]`, so a root-workspace run planned none
of its files. The repository here has the engine's workspace, a member crate,
a standalone crate (`tools/lint`) and a cargo-fuzz crate (`fuzz`): a scope is
split by the workspace each file lies in, each part's diff names its files
relative to that part's root, a mixed scope is two plans, never a refusal,
and a part nothing can run in is said, not failed.
"""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from tests import load

scope = load("mutants-scope.py")
report = load("mutants-report.py")

FILES = {
    "Cargo.toml": '[package]\nname = "rusty"\n\n[workspace]\nmembers = ["crates/a"]\n',
    "crates/a/Cargo.toml": '[package]\nname = "a"\n',
    "crates/a/src/lib.rs": "fn f() -> u32 { 1 }\n",
    "tools/lint/Cargo.toml": '[package]\nname = "lint"\n\n[workspace]\n',
    "tools/lint/src/rules/x.rs": "fn g() -> bool { true }\n",
    "fuzz/Cargo.toml": '[package]\nname = "f"\n[package.metadata]\ncargo-fuzz = true\n\n[workspace]\n',
    "fuzz/fuzz_targets/t.rs": "fn h() {}\n",
}


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, capture_output=True)


class Split(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.addCleanup(os.chdir, os.getcwd())
        self.addCleanup(scope.manifest.cache_clear)
        os.chdir(self.dir.name)
        git("init", "-q", "-b", "main")
        git("config", "user.email", "t@t"), git("config", "user.name", "t")
        for path, text in FILES.items():
            Path(path).parent.mkdir(parents=True, exist_ok=True)
            Path(path).write_text(text)
        git("add", "."), git("commit", "-qm", "base")

    def test_a_standalone_crate_is_its_own_part_with_relative_paths(self) -> None:
        plan, diffs = scope.resolve("tools/lint/src/rules/x.rs", "main")
        self.assertEqual([p["root"] for p in plan["parts"]], ["tools/lint"])
        self.assertIn("+++ b/src/rules/x.rs", diffs["scope-tools-lint.diff"])
        self.assertNotIn("scope-root.diff", diffs)

    def test_a_member_crate_stays_in_the_engine_s_workspace(self) -> None:
        self.assertEqual(scope.workspace("crates/a/src/lib.rs"), ".")
        self.assertEqual(scope.workspace("tools/lint/src/rules/x.rs"), "tools/lint")

    def test_a_mixed_diff_is_two_plans(self) -> None:
        git("checkout", "-qb", "work")
        Path("crates/a/src/lib.rs").write_text("fn f() -> u32 { 2 }\n")
        Path("tools/lint/src/rules/x.rs").write_text("fn g() -> bool { false }\n")
        git("commit", "-qam", "work")
        plan, diffs = scope.resolve("diff", "main")
        self.assertEqual([p["slug"] for p in plan["parts"]], ["root", "tools-lint"])
        self.assertIn("+++ b/crates/a/src/lib.rs", diffs["scope-root.diff"])
        self.assertNotIn("tools/lint", diffs["scope-root.diff"])
        self.assertIn("+++ b/src/rules/x.rs", diffs["scope-tools-lint.diff"])

    def test_a_fuzz_crate_has_nothing_to_run_and_is_not_failed(self) -> None:
        plan, diffs = scope.resolve("fuzz/**", "main")
        self.assertIn("cargo-fuzz", plan["parts"][0]["idle"])
        self.assertEqual(diffs, {})
        line, go = scope.check(plan, {})
        self.assertTrue(go)
        self.assertIn("nothing to run here", line)


PLAN = {"kind": "paths", "scope": "s", "base": "origin/main", "files": ["crates/a/src/lib.rs", "tools/lint/src/x.rs"],
        "parts": [{"root": ".", "slug": "root", "files": ["crates/a/src/lib.rs"]},
                  {"root": "tools/lint", "slug": "tools-lint", "files": ["tools/lint/src/x.rs"]}]}


class Check(unittest.TestCase):
    def plan(self) -> dict:
        return json.loads(json.dumps(PLAN))

    def test_a_half_that_plans_nothing_is_named_and_the_other_runs(self) -> None:
        plan = self.plan()
        line, go = scope.check(plan, {"root": [{"file": "crates/a/src/lib.rs"}]})
        self.assertTrue(go)
        self.assertIn("in 2 plans", line)
        self.assertIn("in `tools/lint/`, its own workspace, 0 mutants planned. None of it", line)
        self.assertEqual([p["planned"] for p in plan["parts"]], [1, 0])

    def test_a_standalone_count_reads_its_own_relative_paths(self) -> None:
        line, _ = scope.check(self.plan(), {"tools-lint": [{"file": "src/x.rs"}] * 2})
        self.assertIn("its own workspace, 2 mutants planned.", line)
        self.assertNotIn("outside", line)

    def test_named_paths_that_plan_nothing_anywhere_are_refused(self) -> None:
        self.assertFalse(scope.check(self.plan(), {})[1])


def outcome(summary: str, name: str) -> dict:
    return {"scenario": {"Mutant": {"name": name}}, "summary": summary}


class Report(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.here = Path(self.dir.name)

    def part(self, slug: str, status: int, *outcomes: dict) -> None:
        out = self.here / f"mutants-{slug}" / "mutants.out"
        out.mkdir(parents=True)
        (out / "outcomes.json").write_text(json.dumps({"outcomes": list(outcomes)}))
        (self.here / f"status-{slug}").write_text(f"{status}\n")

    def test_one_report_covers_both_parts_with_repository_paths(self) -> None:
        self.part("root", 0, outcome("CaughtMutant", "crates/a/src/lib.rs:1:1: replace f"))
        self.part("tools-lint", 2, outcome("MissedMutant", "src/x.rs:1:1: replace g"))
        plan = {"parts": [{**PLAN["parts"][0], "planned": 1}, {**PLAN["parts"][1], "planned": 1}]}
        text, diffs = report.report(plan, "Scoped.", self.here)
        self.assertEqual(text.count("## Mutation on request"), 1)
        self.assertIn("### the engine's workspace", text)
        self.assertIn("### `tools/lint/`", text)
        self.assertIn("tools/lint/src/x.rs:1:1: replace g", text)
        self.assertIn("# tools/lint/src/x.rs:1:1: replace g", diffs)

    def test_a_broken_part_leaves_no_report(self) -> None:
        self.part("tools-lint", 4)
        with self.assertRaises(SystemExit) as stop:
            report.report({"parts": [{**PLAN["parts"][1], "planned": 3}]}, "Scoped.", self.here)
        self.assertIn("`tools-lint` part exited 4", str(stop.exception.code))

    def test_an_unanswered_run_says_why(self) -> None:
        text = report.unanswered("the scope could not be read: no tracked Rust file matches 'x,'\n")
        self.assertTrue(text.startswith("## Mutation on request"))
        self.assertIn("❌ No answer: the scope could not be read", text)


if __name__ == "__main__":
    unittest.main()
