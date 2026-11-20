#!/usr/bin/env python3
"""#2463: `bench-ab` end to end on a fake machine whose process table turns busy mid-run: a
tool gone within the wait lets the legs go on, one that stays stops the run, and the
table is written from the legs that finished."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from bench import refuse
from tests.bench.test_stress_flow import Fake

FMT = "4155958 cargo-fmt\n"


class Busy(Fake):
    """Busy from the `busy_at`-th leg run on, for `polls` reads (`None`: for good); each
    scene leg writes one scene's report, each yard run a `bench.json`."""

    def __init__(self, busy_at: int, polls: int | None):
        super().__init__()
        self.busy_at, self.polls, self.now = busy_at, polls, 0.0

    def runs(self):
        return [e for e in self.log if e[0] == "run"]

    def run(self, argv, **kw):
        if argv[0] != "ps":
            if Path(argv[-1]).parent.name == "stress":
                return super().run(argv, **kw)
            report = Path(argv[-1]) / "yard-0123abcd" / "a.json"
            report.parent.mkdir(parents=True, exist_ok=True)
            avg = 2.0 if "pr-7-head" in argv[0] else 4.0
            report.write_text(json.dumps({"rows": [{"metric": "cpu_ms", "avg": avg}]}))
            self.log.append(("run", argv[0], Path(argv[-1])))
            return subprocess.CompletedProcess(argv, 0, "", "")
        busy = len(self.runs()) >= self.busy_at and self.polls != 0
        if busy and self.polls is not None:
            self.polls -= 1
        return subprocess.CompletedProcess(argv, 0, FMT if busy else "", "")

    def go(self, *argv):
        def sleep(s):
            self.now += s
        with mock.patch.object(refuse, "CLOCK", lambda: self.now), mock.patch.object(refuse, "SLEEP", sleep):
            return self.main("--project", str(Path(tempfile.mkdtemp())), *argv)

    def table(self) -> str:
        return (self.root / "out/bench/ab/pr-7/table.md").read_text()


class Waits(unittest.TestCase):
    def test_a_formatter_gone_within_two_polls_lets_the_third_leg_run(self):
        fake = Busy(busy_at=2, polls=2)
        code, said = fake.go()
        self.assertEqual(code, 0, said)
        self.assertEqual([e[2].name for e in fake.runs()], ["main", "pr", "main-again"])
        self.assertIn("waiting for cargo-fmt (PID 4155958) to end", said)
        self.assertIn(f"quiet after {2 * refuse.POLL:.0f} s", said)
        self.assertNotIn("this run stopped", fake.table())

    def test_busy_at_the_start_past_the_limit_refuses_as_before(self):
        fake = Busy(busy_at=0, polls=None)
        with self.assertRaises(SystemExit) as stop:
            fake.go()
        self.assertIn("refused: a build or a bench runs on the machine", str(stop.exception.code))
        self.assertEqual(fake.runs(), [])

    def test_under_force_it_waits_then_runs_on_noting_it(self):
        fake = Busy(busy_at=2, polls=None)
        code, said = fake.go("--force")
        self.assertEqual(code, 0, said)
        self.assertIn("FORCE=1 ran past: before the main-again leg", fake.table())
        self.assertGreaterEqual(fake.now, refuse.LIMIT, "--force waits too")


class Stopped(unittest.TestCase):
    def test_a_run_stopped_before_its_third_leg_writes_a_table_without_a_floor(self):
        fake = Busy(busy_at=2, polls=None)
        code, said = fake.go()
        self.assertEqual(code, 1, said)
        self.assertEqual([e[2].name for e in fake.runs()], ["main", "pr"])
        text = fake.table()
        self.assertIn("this run stopped** (before the main-again leg: a build or a bench", text)
        self.assertIn("the main-again leg did not finish", text)
        self.assertIn("**no floor** for the scenes", text)
        lead = text.split("**a**", 1)[1]
        self.assertIn("No floor", lead.splitlines()[2], "the lead says so first")
        self.assertIn("| cpu_ms | 4 | 2 | -50.0% (floor unknown) |", text)

    def test_a_run_stopped_inside_the_yard_keeps_the_scenes_and_the_finished_yard_legs(self):
        fake = Busy(busy_at=5, polls=None)  # 3 scene legs, then 2 runs: the stress pr leg's first is in
        code, said = fake.go("--stress", "--runs", "2")
        self.assertEqual(code, 1, said)
        text = fake.table()
        self.assertIn("before a run of the stress pr leg", text)
        self.assertIn("the stress pr, stress main-again legs did not finish", text)
        self.assertIn("**no floor** for the stress yard", text)
        self.assertNotIn("for the scenes", text)
        scenes, yard = text.split("**stress yard**", 1)
        self.assertIn("| cpu_ms | 4 | 2 | -50.0% | +0.0% |", scenes, "the scenes' table keeps its floor")
        self.assertIn("No PR measurement", yard)
        self.assertIn("| cpu_ms | 4 | absent | absent on the PR |", yard)

    def test_a_run_refused_before_any_leg_finished_writes_no_table(self):
        fake = Busy(busy_at=0, polls=None)
        with mock.patch.object(refuse, "busy", side_effect=[[], ["cargo (PID 9)"]] + [["cargo (PID 9)"]] * 999):
            code, said = fake.go()
        self.assertEqual(code, 1, said)
        self.assertIn("no leg finished: no table", said)
        self.assertFalse((fake.root / "out/bench/ab/pr-7/table.md").exists())


if __name__ == "__main__":
    unittest.main()
