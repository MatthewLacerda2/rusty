#!/usr/bin/env python3
"""Each floored module is measured alone, whatever lands in `src/` (#965).

The ratchet's ignore regex is derived from `src/` and the baseline; these tests
build a throwaway tree and match real-looking paths against what it prints.
"""

from __future__ import annotations

import importlib.util
import re
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
REPO = SCRIPTS.parents[1]
_spec = importlib.util.spec_from_file_location("coverage_ignore", SCRIPTS / "coverage-ignore.py")
cov = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(cov)

BASELINE = "# floors\n\napp 95.00\nphysics 97.00\n"


class Tree(unittest.TestCase):
    """`src/` holds the two floored modules, a platform one and a brand-new one."""

    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name)
        for d in ("app", "physics", "render", "brand_new"):
            (self.root / "src" / d).mkdir(parents=True)
        (self.root / "src" / "main.rs").write_text("")

    def ignored(self, module: str, path: str) -> bool:
        dirs = cov.src_dirs(self.root)
        return re.search(cov.ignore_regex(module, cov.floored(BASELINE), dirs), path) is not None

    def test_a_new_directory_is_ignored_without_listing_it(self):
        self.assertTrue(self.ignored("app", "/w/rusty/src/brand_new/lib.rs"))

    def test_the_module_itself_is_measured(self):
        self.assertFalse(self.ignored("app", "/w/rusty/src/app/run.rs"))
        self.assertFalse(self.ignored("app", "/w/rusty/src/app/deep/step.rs"))

    def test_the_other_floored_module_and_the_platform_are_ignored(self):
        self.assertTrue(self.ignored("app", "/w/rusty/src/physics/ray.rs"))
        self.assertTrue(self.ignored("app", "/w/rusty/src/render/mod.rs"))

    def test_top_level_files_and_tools_are_ignored(self):
        self.assertTrue(self.ignored("app", "/w/rusty/src/main.rs"))
        self.assertTrue(self.ignored("app", "/w/rusty/tools/lint/src/main.rs"))

    def test_a_floor_on_a_missing_directory_is_refused(self):
        with self.assertRaises(SystemExit):
            cov.ignore_regex("app", ["app", "gone"], cov.src_dirs(self.root))

    def test_a_module_without_a_floor_is_refused(self):
        with self.assertRaises(SystemExit):
            cov.ignore_regex("render", cov.floored(BASELINE), cov.src_dirs(self.root))


class Baseline(unittest.TestCase):
    def test_comments_and_blanks_are_not_modules(self):
        self.assertEqual(cov.floored(BASELINE), ["app", "physics"])

    def test_the_real_baseline_floors_real_directories(self):
        modules = cov.floored((REPO / cov.BASELINE).read_text())
        dirs = cov.src_dirs(REPO)
        for module in modules:
            with self.subTest(module=module):
                cov.ignore_regex(module, modules, dirs)


class Workflow(unittest.TestCase):
    def test_the_ratchet_types_no_module_list(self):
        text = (SCRIPTS.parent / "workflows" / "coverage.yml").read_text()
        self.assertIn("coverage-ignore.py", text)
        self.assertNotRegex(text, r"(FLOORED|UNFLOORED)=")


if __name__ == "__main__":
    unittest.main()
