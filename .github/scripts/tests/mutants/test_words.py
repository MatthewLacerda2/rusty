#!/usr/bin/env python3
"""A scope is split in one place, its braces expanded, and a moved file is a move (#2201).

`words` splits at spaces and at commas outside `{}` and `[]`, for every caller
(`make mutants-remote`, a direct dispatch, the queue's `Mutation:` line);
`expand` turns a brace into the globs git can read, since git's own glob
magic matches nothing for `src/{a,b}/**`; and a `diff` scope keeps git's
rename detection, so a moved file with one edit plans that edit, not the file.
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from tests import load

scope = load("mutants-scope.py")


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, capture_output=True)


class Words(unittest.TestCase):
    def test_commas_and_spaces_are_both_separators(self) -> None:
        for value in ("a.rs, b.rs", "a.rs,b.rs", "a.rs , b.rs,", "  a.rs\tb.rs "):
            self.assertEqual(scope.words(value), ["a.rs", "b.rs"], value)

    def test_a_comma_inside_braces_or_brackets_is_the_glob_s(self) -> None:
        self.assertEqual(scope.words("src/{a,b}/**, x/[a,b].rs"), ["src/{a,b}/**", "x/[a,b].rs"])

    def test_braces_expand_as_a_shell_would(self) -> None:
        self.assertEqual(scope.expand("c/{x,y/{p,q}}/s/**"), ["c/x/s/**", "c/y/p/s/**", "c/y/q/s/**"])
        self.assertEqual(scope.expand("plain/**"), ["plain/**"])

    def test_an_unbalanced_brace_is_refused_with_why(self) -> None:
        for glob in ("src/{a,b/**", "src/a}/**"):
            with self.assertRaises(SystemExit) as stop:
                scope.expand(glob)
            self.assertIn("brace", str(stop.exception.code), glob)


class Repo(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.addCleanup(os.chdir, os.getcwd())
        os.chdir(self.dir.name)
        git("init", "-q", "-b", "main")
        git("config", "user.email", "t@t"), git("config", "user.name", "t")
        self.body = [f"fn f{n}() -> u32 {{ {n} }}" for n in range(40)]
        for path in ("crates/x/src/a.rs", "crates/y/src/b.rs"):
            Path(path).parent.mkdir(parents=True, exist_ok=True)
            Path(path).write_text("\n".join(self.body) + "\n")
        git("add", "."), git("commit", "-qm", "base")

    def test_a_brace_scope_matches_what_it_names(self) -> None:
        plan, _ = scope.resolve("crates/{x,y}/src/**", "main")
        self.assertEqual(plan["files"], ["crates/x/src/a.rs", "crates/y/src/b.rs"])

    def test_a_brace_alternative_that_matches_nothing_is_refused(self) -> None:
        with self.assertRaises(SystemExit) as stop:
            scope.resolve("crates/{x,gone}/src/**", "main")
        self.assertIn("crates/gone/src/**", str(stop.exception.code))

    def test_a_moved_and_edited_file_plans_the_edit_not_the_file(self) -> None:
        git("checkout", "-qb", "work")
        Path("crates/x/src/b").mkdir()
        git("mv", "crates/x/src/a.rs", "crates/x/src/b/a.rs")
        edited = self.body[:]
        edited[7] = "fn f7() -> u32 { 70 }"
        Path("crates/x/src/b/a.rs").write_text("\n".join(edited) + "\n")
        git("commit", "-qam", "move")
        plan, diffs = scope.resolve("diff", "main")
        diff = diffs["scope-root.diff"]
        self.assertEqual(plan["files"], ["crates/x/src/b/a.rs"])
        self.assertIn("rename to crates/x/src/b/a.rs", diff)
        self.assertIn("+fn f7() -> u32 { 70 }", diff)
        added = [line for line in diff.splitlines() if line.startswith("+") and not line.startswith("+++")]
        self.assertEqual(added, ["+fn f7() -> u32 { 70 }"])


if __name__ == "__main__":
    unittest.main()
