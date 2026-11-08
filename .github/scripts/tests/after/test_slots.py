"""After a merge, at most two runs at once, none lost to a restart (#1934)."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from merging.after import slots, world
from tests.after import fakes


class Dispatch(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.path = Path(self.dir.name, slots.FILE)
        self.said: list[str] = []
        self.now = [1000.0]

    def mutations(self, gh: fakes.GitHub) -> world.Mutations:
        return world.Mutations("o/r", self.path, self.said.append, gh=gh, clock=lambda: self.now[0])

    def test_a_merged_request_is_dispatched_once_on_main_at_its_commit(self) -> None:
        gh = fakes.GitHub({1: fakes.merged(1, sha="abc")})
        after = self.mutations(gh)
        after.merged(1)
        after.pump()
        [sent] = gh.dispatched
        self.assertEqual(sent["ref"], "main")
        self.assertEqual({k: sent["inputs"][k] for k in ("scope", "commit", "pr")}, {"scope": "diff", "commit": "abc", "pr": "1"})

    def test_a_third_waits_until_a_slot_frees(self) -> None:
        gh = fakes.GitHub({n: fakes.merged(n) for n in (1, 2, 3)})
        after = self.mutations(gh)
        for n in (1, 2, 3):
            after.merged(n)
        self.assertEqual([d["inputs"]["pr"] for d in gh.dispatched], ["1", "2"])
        after.pump()
        self.assertEqual(len(gh.dispatched), slots.CAP)
        gh.finish(1)
        after.pump()
        self.assertEqual([d["inputs"]["pr"] for d in gh.dispatched], ["1", "2", "3"])

    def test_a_restart_keeps_the_waiting_and_the_running(self) -> None:
        gh = fakes.GitHub({n: fakes.merged(n) for n in (1, 2, 3)})
        first = self.mutations(gh)
        for n in (1, 2, 3):
            first.merged(n)
        again = self.mutations(gh)
        again.pump()
        self.assertEqual(len(gh.dispatched), 2, "the two running still hold their slots")
        gh.finish(2)
        again.pump()
        self.assertEqual(gh.dispatched[-1]["inputs"]["pr"], "3")

    def test_no_request_or_no_rust_is_not_dispatched(self) -> None:
        gh = fakes.GitHub({1: fakes.merged(1, body="Closes #9"), 2: fakes.merged(2, sha="md")}, files={"md": ["docs/x.md"]})
        after = self.mutations(gh)
        after.merged(1), after.merged(2)
        self.assertEqual((gh.dispatched, slots.load(self.path)), ([], slots.empty()))

    def test_globs_are_dispatched_even_without_rust_in_the_commit(self) -> None:
        gh = fakes.GitHub({1: fakes.merged(1, body="Mutation: src/physics/**", sha="md")}, files={"md": ["docs/x.md"]})
        self.mutations(gh).merged(1)
        self.assertEqual(gh.dispatched[0]["inputs"]["scope"], "src/physics/**")

    def test_a_refusal_waits_first_in_line_then_is_dropped(self) -> None:
        gh = fakes.GitHub({1: fakes.merged(1), 2: fakes.merged(2)})
        gh.refuse = slots.TRIES
        after = self.mutations(gh)
        after.merged(1)
        self.assertEqual(after.state["waiting"][0]["pr"], 1)
        after.merged(2)
        after.pump()
        self.assertEqual([d["inputs"]["pr"] for d in gh.dispatched], ["2"], "dropped after its tries, then the next")
        self.assertTrue(any("dropped" in s for s in self.said))

    def test_a_run_that_never_appears_frees_its_slot_late(self) -> None:
        gh = fakes.GitHub({n: fakes.merged(n) for n in (1, 2, 3)})
        after = self.mutations(gh)
        for n in (1, 2, 3):
            after.merged(n)
        gh.runs.clear()
        after.pump()
        self.assertEqual(len(gh.dispatched), 2)
        self.now[0] += slots.LOST_SECONDS + 1
        after.pump()
        self.assertEqual(len(gh.dispatched), 3)

    def test_github_not_answering_frees_nothing(self) -> None:
        state = {"waiting": [{"pr": 3, "scope": "diff", "commit": "c"}],
                 "running": [{"pr": n, "scope": "diff", "commit": "c", "request": f"r{n}", "since": 0} for n in (1, 2)]}
        changed = slots.pump(state, None, lambda e, r: None, 10**9, lambda s: None)
        self.assertEqual((changed, len(state["running"]), len(state["waiting"])), (False, 2, 1))

    def test_an_unreadable_file_or_entry_is_none(self) -> None:
        self.path.parent.mkdir(parents=True)
        self.path.write_text("{not json")
        self.assertEqual(slots.load(self.path), slots.empty())
        self.path.write_text('{"waiting": [{"pr": "x"}, {"pr": 1, "scope": "diff", "commit": "c"}]}')
        self.assertEqual([e["pr"] for e in slots.load(self.path)["waiting"]], [1])
