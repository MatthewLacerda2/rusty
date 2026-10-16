#!/usr/bin/env python3
"""#973: a path, make verb or namespace the docs name must exist.

A fake repo (a set of tracked paths) stands in for git, so these run offline.
"""

from __future__ import annotations

import unittest

import docrefs

TRACKED = {"src", "src/lib.rs", "src/api", "src/api/nav", "src/api/nav/mod.rs",
           "docs", "docs/api", "docs/api/Transform.md"}
exists = lambda p: p.rstrip("/") in TRACKED
VERBS = {"gates", "scripts"}


def stale(text: str) -> list[str]:
    return docrefs.stale("doc.md", text, exists, VERBS)


class Paths(unittest.TestCase):
    def test_flags_a_missing_source_file(self):
        self.assertEqual(stale("See `src/x.rs` here.\n"), ["doc.md:1: `src/x.rs` does not exist"])

    def test_passes_existing_paths_dirs_and_suffixes(self):
        text = "`src/lib.rs` `src/api/nav/` `src/lib.rs:12` `src/api/nav/mod.rs::register`\n"
        self.assertEqual(stale(text), [])

    def test_passes_placeholders_globs_and_other_projects(self):
        text = "`src/<module>/mod.rs` `src/physics/**` `src/{a,b}.rs` `src/…` `assets/x.glb`\n"
        self.assertEqual(stale(text), [])

    def test_skips_fenced_blocks(self):
        self.assertEqual(stale("```\nrun `src/x.rs`\n```\n"), [])

    def test_an_allow_with_a_reason_on_the_line_above_excuses(self):
        allowed = "<!-- docrefs: allow MCP method names -->\n`tools/list`\n"
        self.assertEqual(stale(allowed), [])
        self.assertEqual(len(stale("<!-- docrefs: allow -->\n`tools/list`\n")), 1)


class Verbs(unittest.TestCase):
    def test_flags_an_unknown_make_verb(self):
        self.assertEqual(stale("Run `make gate`.\n"), ["doc.md:1: `make gate` is not a Makefile target"])

    def test_passes_a_known_verb_with_arguments(self):
        self.assertEqual(stale("`make gates` and `make scripts ARGS=--x`\n"), [])

    def test_reads_targets_from_a_makefile(self):
        self.assertEqual(docrefs.make_verbs("gates: x ## y\n\tcmd\nGATES := a\n"), {"gates"})


class Namespaces(unittest.TestCase):
    def test_flags_a_namespace_without_its_page(self):
        claude = "one stable set of namespaces (`Transform`,\n `Nav`, and the dev-only `Debug`)"
        missing = docrefs.missing_namespaces(claude, exists)
        self.assertEqual(missing, ["CLAUDE.md: namespace `Nav` has no docs/api/Nav.md",
                                   "CLAUDE.md: namespace `Debug` has no docs/api/Debug.md"])

    def test_flags_the_list_going_missing(self):
        self.assertEqual(len(docrefs.missing_namespaces("no list here", exists)), 1)


if __name__ == "__main__":
    unittest.main()
