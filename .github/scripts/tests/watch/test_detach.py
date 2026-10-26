#!/usr/bin/env python3
"""#1450: a stale stop request does not kill a new supervisor, and `--detach`
says "started" only for a supervisor still running.

Real `fcntl` locks in a temporary directory; real child processes for the start.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from merging.watch import detach, ending, lock, supervisor


def stop_file(common: Path) -> Path:
    stop = common / ending.STOP_FILE
    stop.parent.mkdir(parents=True, exist_ok=True)
    stop.touch()
    return stop


class StaleStop(unittest.TestCase):
    def test_a_stop_with_nothing_alive_to_read_it_is_cleared(self):
        with tempfile.TemporaryDirectory() as tmp:
            stop = stop_file(Path(tmp))
            self.assertEqual(detach.clear_stale_stop(Path(tmp)), detach.CLEARED)
            self.assertFalse(stop.exists())
            self.assertIsNone(detach.clear_stale_stop(Path(tmp)))

    def test_a_stop_a_live_watch_may_be_reading_is_honoured(self):
        with tempfile.TemporaryDirectory() as tmp:
            stop = stop_file(Path(tmp))
            held, _ = lock.hold(Path(tmp) / lock.LOCK)
            with held:
                self.assertIsNone(detach.clear_stale_stop(Path(tmp)))
            self.assertTrue(stop.exists())

    def test_a_live_supervisor_refuses_the_new_start_and_keeps_its_stop(self):
        with tempfile.TemporaryDirectory() as tmp:
            stop = stop_file(Path(tmp))
            held, _ = lock.hold(Path(tmp) / supervisor.LOCK)
            with held:
                self.assertIsNotNone(lock.holder(Path(tmp) / supervisor.LOCK))
                second, holder = lock.hold(Path(tmp) / supervisor.LOCK)
                self.assertIsNone(second)
            self.assertTrue(stop.exists())
            self.assertIsNone(lock.holder(Path(tmp) / supervisor.LOCK))


HOLDS = """import fcntl, os, sys, time
f = open(sys.argv[1], "a+"); fcntl.flock(f, fcntl.LOCK_EX)
f.seek(0); f.truncate(); f.write(f"{os.getpid()}\\n"); f.flush()
print("holding", flush=True); time.sleep(30)
"""


class Confirm(unittest.TestCase):
    def start(self, tmp: str, code: str, *args: str) -> subprocess.Popen:
        log = open(Path(tmp, "queue.log"), "a")
        self.addCleanup(log.close)
        child = subprocess.Popen([sys.executable, "-c", code, *args], stdout=log, stderr=subprocess.STDOUT)
        self.addCleanup(child.wait)
        self.addCleanup(child.kill)
        return child

    def test_a_child_that_exits_at_once_is_reported_with_the_log(self):
        with tempfile.TemporaryDirectory() as tmp:
            child = self.start(tmp, "print('supervisor: a stop was requested'); raise SystemExit(6)")
            failed = detach.confirm(child, Path(tmp, "x.lock"), Path(tmp, "queue.log"), seconds=5)
            self.assertIn(f"(PID {child.pid}) exited 6", failed)
            self.assertIn("supervisor: a stop was requested", failed)

    def test_a_child_holding_the_lock_is_confirmed(self):
        with tempfile.TemporaryDirectory() as tmp:
            held = Path(tmp, "x.lock")
            child = self.start(tmp, HOLDS, str(held))
            self.assertIsNone(detach.confirm(child, held, Path(tmp, "queue.log"), seconds=1.5))

    def test_a_running_child_without_the_lock_is_not_confirmed(self):
        with tempfile.TemporaryDirectory() as tmp:
            child = self.start(tmp, "import time; time.sleep(30)")
            failed = detach.confirm(child, Path(tmp, "x.lock"), Path(tmp, "queue.log"), seconds=0.3)
            self.assertIn("does not hold", failed)


if __name__ == "__main__":
    unittest.main()
