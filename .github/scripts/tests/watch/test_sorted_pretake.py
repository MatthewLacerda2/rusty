#!/usr/bin/env python3
"""#1723: the watch's pre-take check leaves in line a head whose conflicts the
take would resolve, hands back the rest, and records each one handed back once.

[`mainline.conflicts`] over a real repository: `main` and two heads, one adding
to a `sorted` list beside `main`'s addition, one editing the same lines.
"""

from __future__ import annotations

import functools
import subprocess
import tempfile
import unittest
from pathlib import Path

from merging.queue.conflict import record
from merging.watch import mainline
from tests.watch.test_select import BOTS, pr

LIST = "// sorted: names only\nmod a;\nmod m;\nmod z;\n"


class PreTake(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.dir, self.log = tmp.name, Path(tmp.name, "conflicts.jsonl")
        self.run_git("init", "-q", "-b", "main")
        self.run_git("config", "user.email", "t@t"), self.run_git("config", "user.name", "t")
        self.commit("main", LIST)
        self.heads = {
            "adds": self.commit("adds", LIST.replace("mod m;\n", "mod m;\nmod q;\n"), base="main"),
            "edits": self.commit("edits", LIST.replace("mod m;\n", "mod n;\n"), base="main"),
        }
        self.run_git("checkout", "-q", "main")
        self.commit("main", LIST.replace("mod m;\n", "mod m;\nmod p;\n"))
        self.run_git("update-ref", "refs/remotes/origin/main", "main")
        self.pulls = [pr(1, head=self.heads["adds"]), pr(2, head=self.heads["edits"])]

    def run_git(self, *args):
        done = subprocess.run(["git", *args], cwd=self.dir, capture_output=True, text=True, check=False)
        return done.returncode, done.stdout

    def commit(self, branch, text, base=None):
        if base:
            self.run_git("checkout", "-q", "-b", branch, base)
        Path(self.dir, "mods.rs").write_text(text)
        self.run_git("add", "."), self.run_git("commit", "-q", "-m", branch)
        return self.run_git("rev-parse", "HEAD")[1].strip()

    def check(self, known=None):
        note = functools.partial(record.note, self.log)
        return mainline.conflicts(self.pulls, self.run_git, {} if known is None else known, BOTS, record=note)

    def test_a_resolvable_head_stays_in_line_and_an_unresolvable_one_is_handed_back(self):
        self.assertEqual(self.check(), {2: ["mods.rs"]})
        [entry] = record.read(self.log)
        self.assertEqual((entry["pull"], entry["head"], entry["ended"]), (2, self.heads["edits"], record.HANDED_BACK))
        self.assertEqual(entry["paths"], {"mods.rs": False})

    def test_a_repeated_poll_records_one_line(self):
        self.check()
        self.check()  # a fresh `known`, as after a relaunch: the file deduplicates the pair
        self.assertEqual(len(record.read(self.log)), 1)


if __name__ == "__main__":
    unittest.main()
