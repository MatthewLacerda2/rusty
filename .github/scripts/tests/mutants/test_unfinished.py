#!/usr/bin/env python3
"""A sweep shard is finished only when it measured the code (#2164).

`mutants-report` marks a commit `sweep-complete` only when no shard is listed
by `mutants-shards.py unfinished`: a shard stopped at its budget, one whose
baseline failed, one with no exit code, and one whose mutants mostly timed
out are all listed; a clean one, one with survivors, and one with a few
timeouts among caught mutants are not.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tests import load, SCRIPTS

shards = load("mutants-shards.py")


class Unfinished(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name)

    def shard(self, k: int, rc: str | None, **lists: int) -> Path:
        path = self.root / f"mutants-shard-{k}"
        path.mkdir()
        if rc is not None:
            (path / "exit-code").write_text(rc + "\n")
        for kind, n in lists.items():
            (path / f"{kind}.txt").write_text("".join(f"m{i}\n" for i in range(n)))
        return path

    def test_finished_shards_are_not_listed(self) -> None:
        self.assertEqual(shards.unfinished(self.shard(0, "0", caught=5)), "")
        self.assertEqual(shards.unfinished(self.shard(1, "2", caught=5, missed=2)), "")
        self.assertEqual(shards.unfinished(self.shard(2, "3", caught=5, timeout=1)), "")

    def test_a_shard_of_mostly_timeouts_did_not_finish(self) -> None:
        self.assertEqual(shards.unfinished(self.shard(0, "3", caught=1, timeout=20)), "timeouts:20/21")

    def test_a_stopped_broken_or_lost_shard_did_not_finish(self) -> None:
        self.assertEqual(shards.unfinished(self.shard(0, "124", caught=5)), "budget")
        self.assertEqual(shards.unfinished(self.shard(1, "baseline")), "exit:baseline")
        self.assertEqual(shards.unfinished(self.shard(2, None)), "exit:none")

    def test_the_cli_names_each_unfinished_shard(self) -> None:
        self.shard(0, "0", caught=3)
        self.shard(1, "3", timeout=4)
        done = subprocess.run(
            [sys.executable, str(SCRIPTS / "mutants-shards.py"), "unfinished",
             "--shards", str(self.root), "--total", "3"],
            capture_output=True, text=True, check=True)
        self.assertEqual(done.stdout, " 1(timeouts:4/4) 2(exit:none)\n")


if __name__ == "__main__":
    unittest.main()
