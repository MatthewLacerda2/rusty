"""#1966: a module's `stored-format` marker makes `make review-level` say `high`."""

from __future__ import annotations

import contextlib
import io
import subprocess
import unittest
from unittest import mock

from dispatch import review_level
from tests import SCRIPTS, load

ROOT = SCRIPTS.parent.parent
cli = load("review-level.py")

#: PR #1962's files (#1958): it changed the `.occlusion` file's fingerprint from
#: `gather.rs` and read `medium` under the hand table.
PR_1962 = ["docs/api/Lighting.md", "src/dev/bake/generate_lighting.rs", "src/dev/bake/pvs_only_tests.rs",
           "src/dev/lua_surface/generate.rs", "src/editor/inspector/assets/scene/bake/mod.rs",
           "src/editor/inspector/assets/scene/bake/occlusion_only_tests.rs",
           "src/editor/inspector/assets/scene/bake/poll.rs", "src/render/scene/occlusion/mod.rs",
           "src/scene/lighting/occlusion/gather.rs", "src/scene/lighting/occlusion/mod.rs",
           "src/scene/lighting/occlusion/print_tests.rs"]


#: What `git grep` prints for PR #1962's case on `origin/main`: the `.occlusion`
#: marker (indented, as the lint allows), one elsewhere, and a line that is no marker.
GREP_1962 = ("origin/main:src/scene/lighting/occlusion/mod.rs://! stored-format: the `<scene>.occlusion` file\n"
             "origin/main:src/scene/io/mod.rs:  //! stored-format: the scene file\n"
             "origin/main:src/x.rs://! no marker\n")


def fake_git(*answers: subprocess.CompletedProcess):
    """`subprocess.run` answering each call with the next of `answers`, recording argv."""
    calls = []

    def run(argv, **_):
        calls.append(argv)
        return answers[len(calls) - 1]
    return run, calls


def grep(code: int, out: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess([], code, out, "")


class Markers(unittest.TestCase):
    def tree(self, *answers) -> tuple[dict[str, str], list, str]:
        run, calls = fake_git(*answers)
        err = io.StringIO()
        with mock.patch.object(review_level.subprocess, "run", run), contextlib.redirect_stderr(err):
            return review_level.tree_markers(), calls, err.getvalue()

    def test_the_occlusion_marker_makes_pr_1962_high(self):
        marked, calls, err = self.tree(grep(0, GREP_1962))
        self.assertIn("origin/main", calls[0])
        self.assertEqual(err, "")
        self.assertEqual(marked, {"src/scene/lighting/occlusion/mod.rs": "the `<scene>.occlusion` file",
                                  "src/scene/io/mod.rs": "the scene file"})
        level, decided = review_level.level(PR_1962, marked)
        self.assertEqual(level, "high")
        self.assertIn("src/scene/lighting/occlusion/gather.rs", [path for _, path in decided])
        self.assertTrue(all(".occlusion" in why for why, _ in decided), decided)

    def test_an_unreadable_main_falls_back_to_the_tree_and_says_so(self):
        marked, calls, err = self.tree(grep(128), grep(0, "src/a/mod.rs://! stored-format: a\n"))
        self.assertEqual(marked, {"src/a/mod.rs": "a"})
        self.assertNotIn("origin/main", calls[1])
        self.assertIn("reading the working tree", err)

    def test_finding_no_marker_says_so(self):
        marked, _, err = self.tree(grep(1))
        self.assertEqual(marked, {})
        self.assertIn("no stored-format marker", err)

    def test_the_tree_still_marks_the_formats_the_table_used_to_list(self):
        # Hand-coded formats (`.occlusion`, `.navmesh`) no lint rule can see: their
        # markers are held here, read straight from the files.
        for owner in ["crates/scene/src/lighting/occlusion/mod.rs", "crates/navigation/src/bake/saved/mod.rs",
                      "crates/scene/src/serialize.rs", "crates/scene/src/io/mod.rs", "crates/scene/src/prefab/mod.rs",
                      "crates/core/src/storage.rs", "crates/asset/src/sidecar/mod.rs", "crates/asset/src/convert/mod.rs",
                      "crates/asset/src/import_cache/mod.rs", "crates/asset/src/animation_graph/mod.rs",
                      "crates/shadergen/src/lib.rs", "src/dev/demo/mod.rs", "crates/scene/src/lighting/io.rs"]:
            with self.subTest(owner):
                lines = (ROOT / owner).read_text().splitlines()
                self.assertTrue(any(review_level.parse_marker(line) for line in lines))

    def test_coverage_reads_the_shared_table_as_the_lint_rule_does(self):
        table = ROOT / "tools/lint/src/rules/stored_format_cover.tsv"
        rows = [r for r in table.read_text().splitlines() if r and not r.startswith("# ")]
        self.assertGreater(len(rows), 5)
        for row in rows:
            path, marked, want = row.split("\t")
            with self.subTest(row):
                got = review_level.covering(path, dict.fromkeys(marked.split(","), "x"))
                self.assertEqual(got is not None, want == "yes")

    def test_a_marker_covers_its_directory_or_its_file(self):
        marked = {"src/a/mod.rs": "a", "src/b.rs": "b", "src/c/d.rs": "d"}
        self.assertEqual(review_level.covering("src/a/x/y.rs", marked), "a")
        self.assertEqual(review_level.covering("src/b/y.rs", marked), "b")
        self.assertEqual(review_level.covering("src/b.rs", marked), "b")
        self.assertEqual(review_level.covering("src/c/d.rs", marked), "d")
        self.assertIsNone(review_level.covering("src/c/e.rs", marked))
        self.assertIsNone(review_level.covering("src/bb/y.rs", marked))
        self.assertEqual(review_level.reasons_for("src/a/z.rs", marked), ["stored format (a)"])

    def test_a_moved_module_is_covered_as_it_was(self):
        marked = {"crates/a/src/lib.rs": "a", "src/b/mod.rs": "b"}
        self.assertEqual(review_level.covering("crates/a/src/x/y.rs", marked), "a")
        self.assertEqual(review_level.covering("crates/b/src/y.rs", marked), "b")
        self.assertIsNone(review_level.covering("crates/a/build.rs", marked))

    def test_a_marker_a_pr_adds_counts(self):
        files = [{"filename": "src/new/mod.rs", "patch": "@@ -1 +1,3 @@\n //! x\n+//!\n+//! stored-format: `.new`"},
                 {"filename": "src/old.rs", "patch": "-//! stored-format: `.old`\n+//! gone"},
                 {"filename": "src/plain.rs", "patch": "+fn f() {}"}, {"filename": "bin.png"}]
        self.assertEqual(review_level.patch_markers(files), {"src/new/mod.rs": "`.new`", "src/old.rs": "`.old`"})

    def test_the_command_line_reads_both(self):
        files = [{"filename": "src/f/x.rs"}, {"filename": "src/g/mod.rs", "patch": "+//! stored-format: g"}]
        gh = lambda path: files if "page=1" in path else []  # noqa: E731
        out = io.StringIO()
        with mock.patch.object(cli.ghrest, "slug", return_value="o/r"), contextlib.redirect_stdout(out):
            code = cli.main(["7"], gh=gh, markers=lambda: {"src/f/mod.rs": "f"})
        self.assertEqual(code, 0)
        self.assertEqual(out.getvalue().splitlines(),
                         ["high", "  stored format (f): src/f/x.rs", "  stored format (g): src/g/mod.rs"])


if __name__ == "__main__":
    unittest.main()
