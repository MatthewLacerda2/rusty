#!/usr/bin/env python3
"""#2381: a timed run refuses another one, a build beside it, and un-imported textures."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from bench import refuse

PS = """    1 systemd
  100 /usr/bin/cargo
  101 rustc
  102 bench-scenes
  103 python3
  104 cargo-nextest
  200 bash
"""


class Lock(unittest.TestCase):
    def setUp(self):
        self.lock = Path(tempfile.mkdtemp()) / "out" / "bench" / ".timed-run.lock"

    def test_a_live_holder_refuses_and_a_dead_one_is_taken_over(self):
        self.assertIsNone(refuse.take_lock(self.lock, 7, lambda pid: True))
        said = refuse.take_lock(self.lock, 8, lambda pid: True)
        self.assertIn("another timed run", said)
        self.assertIn("PID 7", said)
        self.assertIsNone(refuse.take_lock(self.lock, 8, lambda pid: False), "stale")
        self.assertEqual(self.lock.read_text().strip(), "8")

    def test_release_drops_only_its_own_lock(self):
        refuse.take_lock(self.lock, 7, lambda pid: False)
        refuse.release(self.lock, 9)
        self.assertTrue(self.lock.exists())
        refuse.release(self.lock, 7)
        self.assertFalse(self.lock.exists())


class Busy(unittest.TestCase):
    def test_builds_and_benches_are_found_and_this_process_is_not(self):
        self.assertEqual(refuse.busy(PS, 101), ["cargo (PID 100)", "bench-scenes (PID 102)", "cargo-nextest (PID 104)"])
        self.assertIn("runs alone", refuse.busy_refusal(refuse.busy(PS, 0)))
        self.assertIsNone(refuse.busy_refusal(refuse.busy("  1 systemd\n 5 bash\n", 0)))


class Imports(unittest.TestCase):
    def test_pending_textures_refuse_naming_them(self):
        self.assertIsNone(refuse.pending_refusal(0, ""))
        said = refuse.pending_refusal(1, "assets/a.png\nassets/b.png\n")
        self.assertIn("un-imported textures", said)
        self.assertIn("assets/b.png", said)
        self.assertIn("failed (exit 2)", refuse.pending_refusal(2, "cannot open"))


if __name__ == "__main__":
    unittest.main()
