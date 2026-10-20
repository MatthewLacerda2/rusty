#!/usr/bin/env python3
"""#1096: the pre-push hook formats every crate a branch touches, `tools/lint` too.

The flag and pass cases run rustfmt for real, in a throwaway repository whose
`origin/main` is a plain ref: no remote, nothing pushed.
"""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

import resolved

ORDERED = "mod alpha;\nmod beta;\n\nfn main() {}\n"
MISORDERED = "mod beta;\nmod alpha;\n\nfn main() {}\n"
CRATE = '[package]\nname = "{}"\nversion = "0.1.0"\nedition = "2021"\n'


class Checks(unittest.TestCase):
    def test_lint_only_change_formats_only_tools_lint(self):
        self.assertEqual(resolved.checks(["tools/lint/src/rules/mod.rs"]), [resolved.LINT_FMT])

    def test_engine_change_formats_the_root(self):
        self.assertEqual(resolved.checks(["src/main.rs", "docs/x.md"]), [resolved.ROOT_FMT])

    def test_no_rust_change_runs_nothing(self):
        self.assertEqual(resolved.checks(["Makefile", "tools/lint/CLAUDE.md"]), [])

    def test_hook_input_drops_deletes(self):
        sha = "a" * 40
        text = f"refs/heads/x {sha} refs/heads/x {'b' * 40}\n(delete) {resolved.ZERO} refs/heads/y {sha}\n"
        self.assertEqual(resolved.pushed_tips(text), [sha])


class Repo(unittest.TestCase):
    """A two-crate repository: the root crate and a standalone `tools/lint`."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        self.write("Cargo.toml", CRATE.format("root"))
        self.write("src/lib.rs", "pub fn f() {}\n")
        self.write("tools/lint/Cargo.toml", CRATE.format("lint") + "\n[workspace]\n")
        self.write("tools/lint/src/alpha.rs", "pub fn a() {}\n")
        self.write("tools/lint/src/beta.rs", "pub fn b() {}\n")
        self.write("tools/lint/src/main.rs", ORDERED)
        self.git("init", "-q", "-b", "work")
        self.commit("base")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")

    def tearDown(self):
        self.dir.cleanup()

    def write(self, path: str, text: str):
        (self.root / path).parent.mkdir(parents=True, exist_ok=True)
        (self.root / path).write_text(text)

    def git(self, *args: str):
        subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)

    def commit(self, message: str):
        self.git("add", "-A")
        self.git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--no-verify",
                 "-m", message)

    def test_flags_misordered_mod_lines_in_tools_lint(self):
        self.write("tools/lint/src/main.rs", MISORDERED)
        self.commit("keep-both resolution")
        self.assertEqual(resolved.run(["HEAD"], cwd=str(self.root), quiet=True), 1)

    def test_passes_the_same_file_in_rustfmt_order(self):
        self.write("tools/lint/src/main.rs", ORDERED + "\nfn g() {}\n")
        self.commit("formatted")
        self.assertEqual(resolved.run(["HEAD"], cwd=str(self.root), quiet=True), 0)

    def test_a_tip_other_than_head_is_let_through(self):
        self.write("tools/lint/src/main.rs", MISORDERED)
        self.commit("misordered")
        self.git("checkout", "-q", "HEAD~1")
        self.assertEqual(resolved.run(["work"], cwd=str(self.root), quiet=True), 0)


if __name__ == "__main__":
    unittest.main()
