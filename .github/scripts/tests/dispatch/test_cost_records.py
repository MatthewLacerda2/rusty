"""#2452: a record's parts sum to its total, an idle session is read again when its
listing moves, and a kept record that cannot be read is absent, never a traceback."""

from __future__ import annotations

import contextlib
import io
import tempfile
import unittest
from pathlib import Path

from dispatch import cost
from tests.dispatch.test_cost import THREAD, Api, cli, routine


def record(**times) -> dict:
    return {"issue": 7, "kind": "round", "cause": "", "fired": "2026-11-20T02:27:10Z", "turns": 3, **times}


class Split(unittest.TestCase):
    CASES = {  # (init, first_edit, end, gates, gates_reading) -> (sandbox, reading, gates, rest)
        "no edit: every gate is the reading's": ((50, None, 1000, 600, 600), (50, 350, 600, 0)),
        "no edit, kept before the reading's share was": ((50, None, 1000, 600, 0), (50, 350, 600, 0)),
        "no gate": ((50, 150, 1000, 0, 0), (50, 100, 0, 850)),
        "gates before and after the edit": ((50, 400, 1000, 500, 200), (50, 150, 500, 300)),
        "only before the edit": ((50, 400, 1000, 200, 200), (50, 150, 200, 600)),
        "an edit past the end": ((50, 2000, 1000, 100, 100), (50, 850, 100, 0)),
        "more gates than time": ((50, 100, 300, 900, 900), (50, 0, 250, 0)),
    }

    def test_the_four_parts_sum_to_the_total_for_every_record(self):
        for name, ((init, edit, end, gates, before), want) in self.CASES.items():
            parts = cost.split(record(init=init, first_edit=edit, end=end, gates=gates, gates_reading=before))
            self.assertEqual(tuple(parts.values()), want, name)
            self.assertEqual(sum(parts.values()), end, name)

    def test_a_measured_session_with_no_edit_counts_its_gates_once(self):
        done = cost.read([routine(7, "cse_noedit", 12)], {}, NoEdit([routine(7, "cse_noedit", 12)]),
                         lambda n: THREAD)
        parts = cost.split(done.records["cse_noedit"])
        self.assertEqual(parts, {"sandbox": 50, "reading": 360, "gates": 600, "rest": 0})


class NoEdit(Api):
    LOG = "\n".join([
        "[2026-11-20T02:28:00Z] init: model=x cwd=/w",
        '[2026-11-20T02:29:00Z] tool_use Bash: {"command":"git merge origin/main && make gates"}',
        "[2026-11-20T02:39:00Z] tool_result: exit=0",
        "[2026-11-20T02:44:00Z] result: success is_error=false turns=4 duration=990s",
    ])

    def __call__(self, arguments):
        return super().__call__(arguments) if arguments["action"] == "list" else (200, {}, self.LOG)


class Kept(unittest.TestCase):
    def listed(self, finished: str) -> dict:
        found = routine(7, "cse_round", 12)
        found["last_run"]["finished_at"] = finished
        return found

    def test_an_idle_session_is_read_again_when_its_listing_moved(self):
        first = cost.read([self.listed("03:00")], {}, Api([]), lambda n: THREAD).records
        self.assertEqual(first["cse_round"]["listed"], {"status": "SUCCEEDED", "finished_at": "03:00"})
        same = cost.read([self.listed("03:00")], first, Api([]), lambda n: THREAD)
        self.assertEqual((same.records, same.stale), ({}, []))
        moved = cost.read([self.listed("04:00")], first, Api([]), lambda n: THREAD)
        self.assertEqual(moved.stale, ["cse_round"])
        self.assertEqual(moved.records["cse_round"]["listed"]["finished_at"], "04:00")

    def test_a_moved_session_now_running_drops_its_old_record(self):
        busy = {**routine(7, "cse_busy", 12)}
        kept = {"cse_busy": record(init=1, end=2, listed={"status": "SUCCEEDED", "finished_at": None})}
        busy["last_run"]["finished_at"] = "05:00"
        with tempfile.TemporaryDirectory() as home:
            path = Path(home, "c.json")
            cli.state.write(path, kept)
            with contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(cli.main([], gh=lambda p: [], ask=Api([busy]), path=path), 0)
            self.assertEqual(cost.load(path), {})
        self.assertIn("skipped #7 cse_busy: still running", out.getvalue())

    def test_a_thread_github_did_not_answer_is_skipped_not_kept(self):
        done = cost.read([routine(7, "cse_round", 12)], {}, Api([]), lambda n: None)
        self.assertEqual(done.records, {})
        self.assertEqual(done.skipped, ["#7 cse_round: the issue's comments unread"])

    def test_a_prompt_naming_no_brief_is_kept_as_unknown(self):
        found = routine(7, "s", 12)
        found["job_config"] = {}
        self.assertEqual(cost.kind(found, lambda n: None), ("?", "no brief named"))

    def test_a_record_with_a_missing_or_wrong_field_reads_as_absent(self):
        broken = {"a": record(init="50", end=1000), "b": {"end": None}, "c": record(init=10, end=100, gates="x"),
                  "d": record(init=True, end=100), "e": {"kind": 3, "fired": 5}}
        told = cost.report(broken, 0, [])
        self.assertEqual(told.count("times unreadable"), 4)
        self.assertIn("total 100s, sandbox 10s, reading 90s, gates 0s", told)


if __name__ == "__main__":
    unittest.main()
