#!/usr/bin/env python3
"""#1723's review: a binary file both sides changed is handed back by name by the
watch's pre-take check, never a traceback that kills the watch.

Over a real repository, through the watch's own `git` ([`queue.world.merge_tree_git`]),
since the failure was its text decoding of `cat-file -p`.
"""

from __future__ import annotations

import functools
import subprocess
import tempfile
import unittest
from pathlib import Path

from merging.queue import world
from merging.watch import mainline
from tests.watch.test_select import BOTS, pr


class Binary(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.dir = tmp.name
        for args in (("init", "-q", "-b", "main"), ("config", "user.email", "t@t"), ("config", "user.name", "t")):
            self.git(*args)

    def git(self, *args: str) -> str:
        return subprocess.run(["git", *args], cwd=self.dir, capture_output=True, check=True).stdout.decode()

    def commit(self, data: bytes) -> str:
        Path(self.dir, "icon.png").write_bytes(data)
        self.git("add", "."), self.git("commit", "-q", "-m", "c")
        return self.git("rev-parse", "HEAD").strip()

    def test_a_conflicted_binary_file_is_handed_back_naming_it(self):
        self.commit(b"\x89PNG\xff\xfe base")
        self.git("checkout", "-q", "-b", "b")
        head = self.commit(b"\x89PNG\xff\xfe ours \x80")
        self.git("checkout", "-q", "main")
        self.commit(b"\x89PNG\xff\xfe theirs \x81")
        self.git("update-ref", "refs/remotes/origin/main", "main")
        found = mainline.conflicts([pr(1, head=head)], functools.partial(world.merge_tree_git, self.dir), {}, BOTS)
        self.assertEqual(found, {1: ["icon.png"]})


if __name__ == "__main__":
    unittest.main()
