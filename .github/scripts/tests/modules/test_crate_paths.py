"""`make move-crate`'s re-aim of the moved module's `src/<m>/` paths inside the crate."""

from __future__ import annotations

import unittest

from tests import load

where = load("crate_paths.py")


class Paths(unittest.TestCase):
    def test_a_relative_or_make_rooted_path_is_the_modules_too(self):
        for pre in ("", "./", "../", "$(CURDIR)/", "`"):
            self.assertEqual(where.paths(f"{pre}src/audio/x.rs", "audio"), f"{pre}crates/audio/src/x.rs", pre)
        self.assertEqual(where.paths("see src/audio/mod.rs", "audio"), "see crates/audio/src/lib.rs")

    def test_another_crates_like_named_folder_or_a_longer_name_is_left_alone(self):
        for text in ("crates/asset/src/audio/x.rs", "crates/test-support/src/audio/x", "lib-src/audio/x",
                     "src/audiox/y"):
            self.assertEqual(where.paths(text, "audio"), text)


if __name__ == "__main__":
    unittest.main()
