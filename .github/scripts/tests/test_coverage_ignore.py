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


class Moved(unittest.TestCase):
    """`physics` has moved into `crates/`; `app` and `render` have not."""

    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name).resolve()
        for d in ("src/app", "src/render", "crates/physics/src/shape", "crates/test-support/src"):
            (self.root / d).mkdir(parents=True)
        (self.root / "src" / "lib.rs").write_text("")

    def regex(self, module: str) -> str:
        r = self.root
        return cov.ignore_regex(module, ["app", "physics"], cov.src_dirs(r), cov.src_files(r),
                                cov.members(r), r)

    def at(self, path: str) -> str:
        """`path` as llvm-cov reports it: absolute, under the checkout."""
        return f"{self.root.as_posix()}/{path}"

    def test_a_moved_module_is_measured_and_its_top_files_too(self):
        for path in ("crates/physics/src/ray.rs", "crates/physics/src/shape/a.rs"):
            self.assertIsNone(re.search(self.regex("physics"), self.at(path)), path)
        self.assertRegex(self.at("src/app/run.rs"), self.regex("physics"))
        self.assertRegex(self.at("src/lib.rs"), self.regex("physics"))

    def test_a_moved_crates_own_lib_rs_is_not_the_roots(self):
        self.assertIsNone(re.search(self.regex("physics"), self.at("crates/physics/src/lib.rs")))

    def test_a_moved_module_and_the_test_helpers_are_ignored_beside_another(self):
        for path in ("crates/physics/src/ray.rs", "crates/test-support/src/temp.rs"):
            self.assertRegex(self.at(path), self.regex("app"))
        self.assertIsNone(re.search(self.regex("app"), self.at("src/app/run.rs")))

    def test_a_moved_subfolder_named_like_a_root_module_is_refused(self):
        (self.root / "crates/physics/src/render").mkdir()
        with self.assertRaises(SystemExit):
            self.regex("physics")


class Baseline(unittest.TestCase):
    def test_comments_and_blanks_are_not_modules(self):
        self.assertEqual(cov.floored(BASELINE), ["app", "physics"])

    def test_the_real_baseline_floors_real_directories(self):
        modules = cov.floored((REPO / cov.BASELINE).read_text())
        dirs, crates = cov.src_dirs(REPO), cov.members(REPO)
        for module in modules:
            with self.subTest(module=module):
                cov.ignore_regex(module, modules, dirs, crates=crates, root=REPO)


class Workflow(unittest.TestCase):
    def test_the_ratchet_types_no_module_list(self):
        text = (SCRIPTS.parent / "workflows" / "coverage.yml").read_text()
        self.assertIn("coverage-ignore.py", text)
        self.assertNotRegex(text, r"(FLOORED|UNFLOORED)=")


if __name__ == "__main__":
    unittest.main()
