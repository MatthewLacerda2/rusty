#!/usr/bin/env python3
"""A merged pull request's globs narrow its commit's diff, never widen it (#2371).

A `Mutation: <globs>` line used to mutate every matching file whole, which
planned 737 mutants for one merged pull request. With `--commit`, globs keep
the commit's own Rust change to the files they name; by hand, without it,
they still name files whole.
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from tests import load

scope = load("mutants-scope.py")

BIG = "".join(f"fn f{i}() -> u32 {{ {i} }}\n" for i in range(20))


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


class Narrow(unittest.TestCase):
    """`main` holds three files; the commit edits one line of `a.rs`, all of `b.rs`
    is untouched, and `old.rs` is moved to `moved.rs` with one line edited."""

    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.addCleanup(os.chdir, os.getcwd())
        os.chdir(self.dir.name)
        git("init", "-q", "-b", "main")
        git("config", "user.email", "t@t"), git("config", "user.name", "t")
        for f in ("src/physics/a.rs", "src/physics/b.rs", "src/nav/old.rs"):
            Path(f).parent.mkdir(parents=True, exist_ok=True)
            Path(f).write_text(BIG)
        git("add", "."), git("commit", "-qm", "base")
        Path("src/physics/a.rs").write_text(BIG.replace("{ 3 }", "{ 33 }"))
        git("mv", "src/nav/old.rs", "src/nav/moved.rs")
        Path("src/nav/moved.rs").write_text(BIG.replace("{ 5 }", "{ 55 }"))
        git("add", "."), git("commit", "-qm", "work")
        self.sha = git("rev-parse", "HEAD").strip()

    def test_commit_globs_plan_only_changed_lines_of_matching_files(self) -> None:
        plan, diffs = scope.resolve("src/physics/**", "main", commit=self.sha)
        diff = diffs["scope-root.diff"]
        self.assertEqual((plan["kind"], plan["files"]), ("diff", ["src/physics/a.rs"]))
        self.assertIn("+fn f3() -> u32 { 33 }", diff)
        self.assertNotIn("fn f15()", diff, "only the changed line's hunk, not the file whole")
        self.assertNotIn("/dev/null", diff)
        self.assertNotIn("moved.rs", diff, "a changed file the globs do not name is left out")
        line, go = scope.check(plan, {"root": [{"file": "src/physics/a.rs"}]})
        self.assertTrue(go)
        self.assertIn("in the files `src/physics/**` matches", line)

    def test_a_moved_file_keeps_its_rename(self) -> None:
        _, diffs = scope.resolve("src/nav/**", "main", commit=self.sha)
        diff = diffs["scope-root.diff"]
        self.assertIn("rename from src/nav/old.rs", diff)
        self.assertIn("+fn f5() -> u32 { 55 }", diff)
        self.assertNotIn("fn f15()", diff, "a moved file with one edit is one hunk, not wholly added")

    def test_globs_matching_no_changed_file_plan_nothing_and_say_so(self) -> None:
        plan, diffs = scope.resolve("src/physics/b.rs", "main", commit=self.sha)
        self.assertEqual((plan["files"], plan["parts"], diffs), ([], [], {}))
        line, go = scope.check(plan, {})
        self.assertTrue(go, "an answer, not a refusal")
        self.assertIn("0 mutants planned", line)
        self.assertIn("`src/physics/b.rs` matches no file the commit changed", line)

    def test_one_unchanged_glob_is_named_beside_one_that_matched(self) -> None:
        plan, _ = scope.resolve("src/physics/a.rs, src/physics/b.rs", "main", commit=self.sha)
        self.assertEqual(plan["unchanged"], ["src/physics/b.rs"])
        line, _ = scope.check(plan, {"root": [{"file": "src/physics/a.rs"}]})
        self.assertIn("`src/physics/b.rs` matches no file", line)

    def test_a_hand_ask_keeps_whole_files(self) -> None:
        plan, diffs = scope.resolve("src/physics/**", "main")
        self.assertEqual((plan["kind"], plan["files"]), ("paths", ["src/physics/a.rs", "src/physics/b.rs"]))
        self.assertIn("--- /dev/null", diffs["scope-root.diff"])
        self.assertNotIn("globs", plan)

    def test_a_commit_glob_naming_no_tracked_file_is_still_refused(self) -> None:
        with self.assertRaises(SystemExit):
            scope.resolve("src/nowhere/**", "main", commit=self.sha)


if __name__ == "__main__":
    unittest.main()
