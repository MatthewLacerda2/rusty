"""#1876: `make queue-events` passes what the orchestrator acts on, from 30 lines
back, and ends with status 1 once the supervisor is not running."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from merging.view import events

PASSED = [
    "queue: #1873: merged.",
    "queue: HANDED BACK #1870: CI is red on abc1234. Push a fix; the watch carries on.",
    "supervisor: the watch exited 4: another watch holds the lock; stopped.",
    "supervisor: crashed: OSError: boom; stopped.",
    "queue: watch: #1870 turned ready.",
    "queue: watch: #1871 opened as ready.",
    "queue: watch: #1871 back to draft.",
    "queue: watch: STOPPED: main's tree is not the train's.",
    "queue: after: GitHub refused #5's mutation run 3 times; dropped: HTTP 422: no",
    "queue: after: could not save the mutation requests: disk full",
    "queue: after: 1 unreadable waiting mutation request(s) dropped.",
    "queue: HELD #12: Dependabot edits .github/workflows/ci.yml, which a pull-request run does not prove.",
]

DROPPED = [
    "queue: #1870: ci run 123 in_progress (4 min); waiting.",
    "queue: watch: #1871 opened as a draft.",
    "queue: watch: #1871 queued.",
    "queue: watch: taking #1870.",
    "queue: after: dispatched #5's mutation run (diff) as pr5-abc123.",
    "queue: after: GitHub did not answer for #5's mutation run; asking again next pass: HTTP 502",
    "supervisor: starting the watch.",
    "supervisor: the watch exited 0: its deadline; starting it again in 0 minute(s).",
]


class Keep(unittest.TestCase):
    def test_passes_what_someone_acts_on(self):
        for line in PASSED:
            self.assertTrue(events.keep(line, set()), line)

    def test_drops_the_rest(self):
        for line in DROPPED:
            self.assertFalse(events.keep(line, set()), line)

    def test_a_merge_is_said_once(self):
        merged: set[int] = set()
        self.assertTrue(events.keep("queue: #5: merged.", merged))
        self.assertFalse(events.keep("queue: #5: merged — landed with the train abc1234.", merged))
        self.assertTrue(events.keep("queue: #6: merged — landed with the train abc1234.", merged))


class Follow(unittest.TestCase):
    def run_stream(self, log: Path, alive_polls: int, append=None, comments=()):
        said, calls = [], {"n": 0}

        def running():
            calls["n"] += 1
            return (calls["n"] <= alive_polls), ["queue-status: no supervisor running: …"]

        clock = iter(range(0, 10_000, events.LIVENESS_SECONDS))

        def sleep(_):
            if append:
                append()
        status = events.follow(events.Tail(log), running, lambda: list(comments), said.append, sleep, lambda: next(clock))
        return status, said

    def test_replays_thirty_lines_back_then_ends_when_the_supervisor_dies(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp, "queue.log")
            lines = ["queue: #1: merged."] + [f"queue: #2: ci run {i} in_progress." for i in range(30)] + ["queue: #3: merged."]
            log.write_text("\n".join(lines) + "\n")
            wrote = []

            def append():
                if not wrote:
                    wrote.append(1)
                    with open(log, "a") as out:
                        out.write("queue: watch: #4 turned ready.\nqueue: #4: ci run 9 in_progress.\nqueue: HANDED BA")
            status, said = self.run_stream(log, alive_polls=2, append=append, comments=["STANDING DOWN #9 (me): Standing down"])
            self.assertEqual(status, 1)
            self.assertNotIn("queue: #1: merged.", said)  # 31 lines back: outside the replay
            self.assertEqual(said[0], "queue: #3: merged.")
            self.assertIn("queue: watch: #4 turned ready.", said)
            self.assertIn("STANDING DOWN #9 (me): Standing down", said)
            self.assertFalse(any("HANDED BA" in s for s in said))  # half a line waits for its end
            self.assertTrue(said[-1].startswith("queue-events: the supervisor is not running"))

    def test_a_dead_supervisor_at_the_start_ends_at_once(self):
        with tempfile.TemporaryDirectory() as tmp:
            status, said = self.run_stream(Path(tmp, "missing.log"), alive_polls=0)
            self.assertEqual(status, 1)
            self.assertEqual(len(said), 2)


if __name__ == "__main__":
    unittest.main()
