#!/usr/bin/env python3
"""Each floored module is measured alone, whatever lands in `crates/` (#965).

The ratchet's ignore regex is derived from `crates/` and the baseline; these tests
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
    """`crates/` holds the two floored modules, a platform one, a brand-new one and
    the test helpers; the root package has its `src/lib.rs` and a binary."""

    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name).resolve()
        for d in ("app", "physics", "render", "brand_new", "test-support"):
            (self.root / "crates" / d / "src").mkdir(parents=True, exist_ok=True)
        (self.root / "crates/physics/src/shape").mkdir(parents=True)
        (self.root / "src/bin").mkdir(parents=True)

    def regex(self, module: str) -> str:
        return cov.ignore_regex(module, cov.floored(BASELINE), cov.members(self.root), self.root)

    def ignored(self, module: str, path: str) -> bool:
        """Whether `path`, as llvm-cov reports it (absolute, under the checkout), is ignored."""
        return re.search(self.regex(module), f"{self.root.as_posix()}/{path}") is not None

    def test_a_new_crate_is_ignored_without_listing_it(self):
        self.assertTrue(self.ignored("app", "crates/brand_new/src/lib.rs"))

    def test_the_module_itself_is_measured_its_lib_rs_too(self):
        for path in ("crates/physics/src/ray.rs", "crates/physics/src/shape/a.rs",
                     "crates/physics/src/lib.rs"):
            self.assertFalse(self.ignored("physics", path), path)

    def test_the_other_floored_module_the_platform_and_the_helpers_are_ignored(self):
        for path in ("crates/physics/src/ray.rs", "crates/render/src/lib.rs",
                     "crates/test-support/src/temp.rs"):
            self.assertTrue(self.ignored("app", path), path)

    def test_the_root_package_and_tools_are_ignored(self):
        for path in ("src/lib.rs", "src/bin/play.rs", "tools/lint/src/main.rs"):
            self.assertTrue(self.ignored("app", path), path)

    def test_a_folder_named_like_the_root_packages_is_not_read_as_it(self):
        self.assertFalse(self.ignored("physics", "crates/physics/src/tools/a.rs"))

    def test_a_floor_on_a_missing_crate_is_refused(self):
        with self.assertRaises(SystemExit):
            cov.ignore_regex("app", ["app", "gone"], cov.members(self.root), self.root)

    def test_a_module_without_a_floor_is_refused(self):
        with self.assertRaises(SystemExit):
            self.regex("render")


class Halves(unittest.TestCase):
    """`script` is one crate holding two floored folders, `api` and `scripting`."""

    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name).resolve()
        for d in ("crates/app/src", "crates/script/src/api", "crates/script/src/scripting"):
            (self.root / d).mkdir(parents=True)
        (self.root / "crates/script/src/lib.rs").write_text("")

    def regex(self, module: str) -> str:
        r = self.root
        floors = ["app", "script/api", "script/scripting"]
        return cov.ignore_regex(module, floors, cov.members(r), r, cov.entries(r, "script"))

    def at(self, path: str) -> str:
        return f"{self.root.as_posix()}/{path}"

    def test_a_half_is_measured_without_the_other_half(self):
        self.assertIsNone(re.search(self.regex("script/api"), self.at("crates/script/src/api/a.rs")))
        self.assertRegex(self.at("crates/script/src/scripting/vm.rs"), self.regex("script/api"))
        self.assertRegex(self.at("crates/script/src/lib.rs"), self.regex("script/api"))

    def test_a_half_with_a_nested_folder_named_like_its_sibling_stays_measured(self):
        path = self.at("crates/script/src/scripting/api/bind.rs")
        self.assertIsNone(re.search(self.regex("script/scripting"), path))

    def test_the_halves_are_ignored_beside_another_module(self):
        self.assertRegex(self.at("crates/script/src/api/a.rs"), self.regex("app"))

    def test_a_half_the_crate_does_not_hold_is_refused(self):
        r = self.root
        with self.assertRaises(SystemExit):
            cov.ignore_regex("script/gone", ["script/gone"], cov.members(r), r,
                             cov.entries(r, "script"))


class Baseline(unittest.TestCase):
    def test_comments_and_blanks_are_not_modules(self):
        self.assertEqual(cov.floored(BASELINE), ["app", "physics"])

    def test_the_real_baseline_floors_real_directories(self):
        modules = cov.floored((REPO / cov.BASELINE).read_text())
        crates = cov.members(REPO)
        for module in modules:
            with self.subTest(module=module):
                inside = cov.entries(REPO, module.partition("/")[0])
                cov.ignore_regex(module, modules, crates, REPO, inside)


class Workflow(unittest.TestCase):
    def test_the_ratchet_types_no_module_list(self):
        text = (SCRIPTS.parent / "workflows" / "coverage.yml").read_text()
        self.assertIn("coverage-ignore.py", text)
        self.assertNotRegex(text, r"(FLOORED|UNFLOORED)=")


if __name__ == "__main__":
    unittest.main()
