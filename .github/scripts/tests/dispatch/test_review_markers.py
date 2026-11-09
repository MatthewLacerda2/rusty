"""#1966: a module's `stored-format` marker makes `make review-level` say `high`."""

from __future__ import annotations

import contextlib
import io
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


class Markers(unittest.TestCase):
    def test_the_occlusion_marker_makes_pr_1962_high(self):
        marked = review_level.tree_markers(None, cwd=ROOT)
        level, decided = review_level.level(PR_1962, marked)
        self.assertEqual(level, "high")
        self.assertIn("src/scene/lighting/occlusion/gather.rs", [path for _, path in decided])
        self.assertTrue(all(".occlusion" in why for why, _ in decided), decided)

    def test_the_tree_marks_the_formats_the_table_used_to_list(self):
        marked = review_level.tree_markers(None, cwd=ROOT)
        for path in ["src/scene/serialize.rs", "src/scene/io/mod.rs", "src/scene/prefab/apply.rs",
                     "src/core/storage.rs", "src/asset/sidecar/mod.rs", "src/asset/convert/glb/mod.rs",
                     "src/asset/import_cache/hulls/file.rs", "src/asset/animation_graph/io.rs",
                     "src/shadergen/params.rs", "src/dev/demo/read.rs", "src/scene/lighting/io.rs",
                     "src/navigation/bake/saved/file.rs"]:
            with self.subTest(path):
                self.assertIsNotNone(review_level.covering(path, marked))

    def test_a_marker_covers_its_directory_or_its_file(self):
        marked = {"src/a/mod.rs": "a", "src/b.rs": "b", "src/c/d.rs": "d"}
        self.assertEqual(review_level.covering("src/a/x/y.rs", marked), "a")
        self.assertEqual(review_level.covering("src/b/y.rs", marked), "b")
        self.assertEqual(review_level.covering("src/b.rs", marked), "b")
        self.assertEqual(review_level.covering("src/c/d.rs", marked), "d")
        self.assertIsNone(review_level.covering("src/c/e.rs", marked))
        self.assertIsNone(review_level.covering("src/bb/y.rs", marked))
        self.assertEqual(review_level.reasons_for("src/a/z.rs", marked), ["stored format (a)"])

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
