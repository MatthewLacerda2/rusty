"""A blip, a failed read, a second writer or a bad file loses no request (#1934's review)."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from merging.after import slots, world
from tests.after import fakes


class Faults(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.path = Path(self.dir.name, slots.FILE)
        self.said: list[str] = []

    def mutations(self, gh: fakes.GitHub) -> world.Mutations:
        return world.Mutations("o/r", self.path, self.said.append, gh=gh, clock=lambda: 1000.0)

    def test_a_network_blip_burns_no_tries(self) -> None:
        gh = fakes.GitHub({1: fakes.merged(1)})
        gh.refuse, gh.refusal = slots.TRIES * 3, "HTTP 502: Bad Gateway"
        after = self.mutations(gh)
        after.merged(1)
        for _ in range(slots.TRIES * 3 - 1):
            after.pump()
        self.assertEqual((gh.dispatched, after.state["waiting"][0].get("tries", 0)), ([], 0))
        self.assertEqual(sum("did not answer" in s for s in self.said), 1, "said once, not every pass")
        after.pump()
        self.assertEqual(gh.dispatched[0]["inputs"]["pr"], "1")

    def test_a_failed_read_after_the_merge_is_read_again(self) -> None:
        gh = fakes.GitHub({1: fakes.merged(1)})
        gh.down = 4
        after = self.mutations(gh)
        after.merged(1)
        self.assertEqual(slots.load(self.path)["unread"][0]["pr"], 1, "the number is kept")
        for _ in range(4):
            after.pump()
        self.assertEqual(gh.dispatched[0]["inputs"]["pr"], "1")
        self.assertEqual(slots.load(self.path)["unread"], [])

    def test_a_pull_request_github_keeps_refusing_is_dropped_and_said(self) -> None:
        gh = fakes.GitHub({})
        after = self.mutations(gh)
        after.merged(7)
        for _ in range(slots.TRIES):
            after.pump()
        self.assertEqual(slots.load(self.path), slots.empty())
        self.assertTrue(any("dropped" in s and "#7" in s for s in self.said))

    def test_two_writers_share_the_cap_and_the_file(self) -> None:
        gh = fakes.GitHub({n: fakes.merged(n) for n in (1, 2, 4)})
        watch, named = self.mutations(gh), self.mutations(gh)
        watch.merged(1)
        named.merged(4)
        watch.merged(2)
        self.assertEqual([d["inputs"]["pr"] for d in gh.dispatched], ["1", "4"])
        state = slots.load(self.path)
        self.assertEqual(([e["pr"] for e in state["running"]], [e["pr"] for e in state["waiting"]]), ([1, 4], [2]))

    def test_a_malformed_file_never_raises(self) -> None:
        self.path.parent.mkdir(parents=True)
        gh = fakes.GitHub({})
        for text in ('{"waiting": 5}', '{"running": [{"pr": 1, "scope": "diff", "commit": "c", "request": "r", "since": null}]}',
                     '{"unread": [{"pr": true}], "waiting": [{"pr": 2, "scope": "diff", "commit": "c", "tries": "x"}]}',
                     "[1]", "{not json"):
            self.path.write_text(text)
            self.said.clear()
            self.mutations(gh).pump()
            self.assertEqual(slots.load(self.path), slots.empty(), text)
            self.assertTrue(any("unreadable" in s for s in self.said), text)
            self.said.clear()
            self.mutations(gh).pump()
            self.assertEqual(self.said, [], "the cleaned file was written back, so it is said once")


if __name__ == "__main__":
    unittest.main()
