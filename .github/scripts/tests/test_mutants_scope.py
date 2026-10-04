#!/usr/bin/env python3
"""A scope names exactly what gets mutated, and an empty answer is not clean (#750).

`resolve` is tested against a throwaway git repository, because what it reads
is git's own glob dialect and merge base; `check` is pure.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "mutants-scope.py"
_spec = importlib.util.spec_from_file_location("mutants_scope", SCRIPT)
scope = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(scope)


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, capture_output=True)


class Repo(unittest.TestCase):
    """`main` holds two sim files; the branch edits one and adds a test."""

    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(os.chdir, os.getcwd())
        os.chdir(self.dir.name)
        git("init", "-q", "-b", "main")
        git("config", "user.email", "t@t"), git("config", "user.name", "t")
        for f in ("src/physics/a.rs", "src/navigation/b.rs", "src/navigation/deep/c.rs"):
            Path(f).parent.mkdir(parents=True, exist_ok=True)
            Path(f).write_text("fn f() -> u32 { 1 }\n")
        git("add", "."), git("commit", "-qm", "base")
        git("checkout", "-qb", "work")
        Path("src/physics/a.rs").write_text("fn f() -> u32 { 2 }\n")
        Path("notes.md").write_text("not rust\n")
        git("add", "."), git("commit", "-qm", "work")

    def tearDown(self) -> None:
        self.dir.cleanup()

    def test_diff_is_the_branch_rust_against_the_base(self) -> None:
        plan, diff = scope.resolve("diff", "main")
        self.assertEqual(plan["files"], ["src/physics/a.rs"])
        self.assertIn("+fn f() -> u32 { 2 }", diff)
        self.assertNotIn("notes.md", diff)

    def test_globs_add_the_named_files_whole(self) -> None:
        plan, diff = scope.resolve("src/navigation/*", "main")
        self.assertEqual(plan["files"], ["src/navigation/b.rs"], "`*` stops at a directory")
        self.assertIn("--- /dev/null", diff)
        self.assertIn("+++ b/src/navigation/b.rs", diff)
        plan, _ = scope.resolve("src/navigation/**", "main")
        self.assertEqual(len(plan["files"]), 2, "`**` crosses directories")

    def test_a_glob_matching_nothing_is_refused(self) -> None:
        with self.assertRaises(SystemExit):
            scope.resolve("src/nowhere/**", "main")

    def test_a_missing_base_is_refused_not_read_as_empty(self) -> None:
        with self.assertRaises(SystemExit):
            scope.resolve("diff", "origin/absent")


class Check(unittest.TestCase):
    PATHS = {"kind": "paths", "scope": "src/physics/**", "base": "origin/main",
             "files": ["src/physics/a.rs"]}

    def test_counts_what_was_planned(self) -> None:
        line, go = scope.check(self.PATHS, [{"file": "src/physics/a.rs"}] * 3)
        self.assertTrue(go)
        self.assertIn("3 mutants planned", line)
        self.assertNotIn("outside", line)

    def test_names_mutants_from_outside_the_scope(self) -> None:
        line, _ = scope.check(self.PATHS, [{"file": "src/physics/a.rs"}, {"file": "src/app/x.rs"}])
        self.assertIn("outside that scope (`src/app/x.rs`)", line)

    def test_named_paths_with_nothing_to_mutate_are_refused(self) -> None:
        _, go = scope.check(self.PATHS, [])
        self.assertFalse(go)

    def test_a_diff_with_nothing_to_mutate_is_an_answer(self) -> None:
        line, go = scope.check({**self.PATHS, "kind": "diff", "files": []}, [])
        self.assertTrue(go)
        self.assertIn("0 mutants planned", line)

    def test_an_empty_scope_is_refused(self) -> None:
        with self.assertRaises(SystemExit):
            scope.kind("  ")


if __name__ == "__main__":
    unittest.main()
