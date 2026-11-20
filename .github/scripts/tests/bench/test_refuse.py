#!/usr/bin/env python3
"""#2381: a timed run refuses another one, a build beside it, and un-imported textures."""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path

from bench import refuse

PS = """    1 systemd
  100 /usr/bin/cargo
  101 rustc
  102 bench-scenes
  103 python3
  105 play
  104 cargo-nextest
  200 bash
"""


class Lock(unittest.TestCase):
    def setUp(self):
        self.lock = Path(tempfile.mkdtemp()) / "out" / "bench" / ".timed-run.lock"

    def test_a_live_holder_refuses_naming_its_pid(self):
        held, said = refuse.hold(self.lock)
        self.assertIsNone(said)
        try:
            again, said = refuse.hold(self.lock)
            self.assertIsNone(again)
            self.assertIn("another timed run", said)
            self.assertIn(f"PID {os.getpid()}", said)
        finally:
            held.close()

    def test_a_lock_left_by_a_dead_holder_is_taken(self):
        self.lock.parent.mkdir(parents=True)
        self.lock.write_text("4194301\n")  # a holder that died (or a PID reused since a power loss)
        held, said = refuse.hold(self.lock)
        self.assertIsNone(said, "nothing holds the flock, whatever the file says")
        held.close()
        self.assertIsNone(refuse.hold(self.lock)[1], "and closing the handle drops it")


class Flags(unittest.TestCase):
    OLD = "usage: bench-scenes --project <dir> [--scenes <a,b>] [--before <dir>] <out_dir>"
    NEW = OLD + " [--set <k=v>]... | --project <dir> --pending-imports"

    def test_an_old_binary_cannot_check_textures_nor_take_a_set(self):
        hard, soft = refuse.flags_refusal(self.OLD, Path("/m/bench-scenes"), ())
        self.assertIsNone(hard)
        self.assertIn("could not run", soft)
        hard, _ = refuse.flags_refusal(self.OLD, Path("/m/bench-scenes"), ("a=1",))
        self.assertIn("no --set", hard)

    def test_a_new_binary_passes(self):
        self.assertEqual(refuse.flags_refusal(self.NEW, Path("/p/bench-scenes"), ("a=1",)), (None, None))


class Busy(unittest.TestCase):
    def test_builds_and_benches_are_found_and_this_process_is_not(self):
        self.assertEqual(refuse.busy(PS, 101), ["cargo (PID 100)", "bench-scenes (PID 102)", "play (PID 105)",
                                                    "cargo-nextest (PID 104)"])
        self.assertIn("runs alone", refuse.busy_refusal(refuse.busy(PS, 0)))
        self.assertIsNone(refuse.busy_refusal(refuse.busy("  1 systemd\n 5 bash\n", 0)))


class Imports(unittest.TestCase):
    def test_pending_textures_refuse_naming_them(self):
        self.assertEqual(refuse.pending_answer("main", 0, ""), ("clean", None))
        kind, said = refuse.pending_answer("main", 1, "assets/a.png\nassets/b.png\n")
        self.assertEqual(kind, "pending")
        self.assertIn("main's build finds un-imported textures", said)
        self.assertIn("assets/b.png", said)
        self.assertIn("failed (exit 2)", refuse.pending_answer("main", 2, "cannot open")[1])


if __name__ == "__main__":
    unittest.main()
