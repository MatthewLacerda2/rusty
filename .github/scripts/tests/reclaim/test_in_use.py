#!/usr/bin/env python3
"""#2217: a finished worktree something is using is kept and named, on a real disk and process table.

A process standing in it or running a program from it, or a write inside the
idle window, keeps it; the same tree idle for longer, with no process, goes.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

import reclaim_use as use
from tests.reclaim.fixtures import Worktree, reclaim

HOUR_AGO = time.time() - 2 * 3600


def age(tree: Path, when: float) -> None:
    """Every entry under `tree`, and `tree` itself, last written at `when`."""
    for path in sorted(tree.rglob("*"), key=lambda p: -len(p.parts)) + [tree]:
        os.utime(path, (when, when), follow_symlinks=False)


class InUse(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp()).resolve()
        self.addCleanup(shutil.rmtree, self.root)
        self.tree = self.root / ".claude" / "worktrees" / "local-experiments"
        (self.tree / "target" / "release" / "deps").mkdir(parents=True)
        (self.tree / "src").mkdir()
        (self.tree / "src" / "main.rs").write_text("fn main() {}\n")
        age(self.tree, HOUR_AGO)
        self.wt = Worktree(self.tree, "aaaa")  # detached at a commit on origin/main: finished

    def judge(self, locked: str | None = None) -> tuple[str, str]:
        clean = lambda argv, **_: subprocess.CompletedProcess(argv, 0, "", "")
        machine = reclaim.Machine(clean)
        machine.alive = lambda pid: True
        self.wt.locked = locked
        return reclaim.verdict(self.wt, machine.facts(self.wt, self.root, machine.processes()))

    def spawn(self, program: str, cwd: Path) -> subprocess.Popen:
        child = subprocess.Popen([program, "30"], cwd=cwd)
        self.addCleanup(child.wait)
        self.addCleanup(child.kill)
        time.sleep(0.2)
        return child

    def test_idle_with_no_process_is_removed(self):
        self.assertEqual(self.judge(), ("remove", "detached at a commit on origin/main"))

    def test_a_process_standing_in_it_is_kept_and_named(self):
        child = self.spawn("sleep", self.tree / "src")
        action, why = self.judge()
        self.assertEqual(action, "protect")
        self.assertIn(f"in use by pid {child.pid} (`sleep`)", why)

    def test_a_program_run_from_it_keeps_it_whatever_the_cwd(self):
        binary = self.tree / "target" / "release" / "bench"
        shutil.copy2(shutil.which("sleep"), binary)
        child = self.spawn(str(binary), self.root)
        action, why = self.judge()
        self.assertEqual(action, "protect")
        self.assertIn(f"pid {child.pid}", why)

    def test_written_five_minutes_ago_is_kept(self):
        age(self.tree / "src", time.time() - 5 * 60)
        self.assertEqual(self.judge(), ("protect", "detached at a commit on origin/main, but written 5 minute(s)"
                                        " ago; `--idle 0` to remove it anyway"))

    def test_a_build_landing_in_a_bulk_directory_shows(self):
        (self.tree / "target" / "release" / "deps" / "libx-1.rlib").write_bytes(b"")
        self.assertEqual(self.judge()[0], "protect")

    def test_a_lock_still_keeps_it(self):
        self.assertIn("locked (claude agent", self.judge(locked="claude agent agent-x (pid 1 start 2)")[1])

    def test_the_probe_stops_at_its_depth(self):
        deep = self.tree / "out" / "run" / "1" / "2"
        deep.mkdir(parents=True)
        (deep / "log.txt").write_text("x")
        age(self.tree, HOUR_AGO)
        os.utime(deep / "log.txt")  # rewritten in place, five levels down: the documented miss
        self.assertIsNone(use.newest_write(self.tree, time.time() - 60))
        os.utime(deep)  # a file created or removed four levels down shows
        self.assertIsNotNone(use.newest_write(self.tree, time.time() - 60))


class Arguments(unittest.TestCase):
    def test_idle_window_and_dry_run(self):
        self.assertEqual(reclaim.parse_args([]), (False, 60))
        self.assertEqual(reclaim.parse_args(["--idle", "0", "--dry-run"]), (True, 0))

    def test_a_bad_argument_is_refused(self):
        for argv in (["--idle"], ["--idle", "soon"], ["--force"]):
            with self.assertRaises(SystemExit, msg=argv):
                reclaim.parse_args(argv)

    def test_idle_zero_turns_the_write_check_off(self):
        self.assertIsNone(reclaim.Machine().written(Path(tempfile.gettempdir()), 0))


class Lsof(unittest.TestCase):
    def test_reads_cwd_and_program_with_the_command(self):
        out = "p12\ncsleep\nfcwd\nn/a/b\nftxt\nn/a/b/bin\np13\ncsh\nfcwd\nn/c\n"
        self.assertEqual(use.parse_lsof(out), [(12, Path("/a/b"), "sleep"), (12, Path("/a/b/bin"), "sleep"),
                                               (13, Path("/c"), "sh")])

    def test_no_lsof_output_is_unknown_not_empty(self):
        if Path("/proc/self/cwd").exists():
            self.skipTest("Linux reads /proc, not lsof")
        answer = lambda *argv: subprocess.CompletedProcess(argv, 1, "", "")
        self.assertIsNone(use.processes(answer))


if __name__ == "__main__":
    unittest.main()
