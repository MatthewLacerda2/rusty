#!/usr/bin/env python3
"""#664: the `--watch` loop's endings, driven with fake effects and a fake clock.

It merges what turns ready one at a time, skips a hand-back and remembers
that head (#751, `test_queue_watch_handback.py`), stops on the machine's
failure without blaming the branch, ends when nothing is left, and leaves
named-PR mode as it was. `status` is the exit status the watch ends with.
"""

from __future__ import annotations

import argparse
import importlib.util
import tempfile
import unittest
import unittest.mock
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
_spec = importlib.util.spec_from_file_location("merge_queue_watch", HERE / "merge-queue.py")
queue = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(queue)
watch = queue.queue_watch


def pr(number: int, draft=False, head=None) -> dict:
    return {"number": number, "isDraft": draft, "headRefOid": head or f"sha{number}", "author": {"login": "x"}}


class World:
    """GitHub as a script: `listings` are returned in turn (the last repeats),
    and `verdicts[n]` is what taking pull request `n` ends in."""

    def __init__(self, tmp: str, listings: list, verdicts: dict[int, str]):
        self.listings, self.verdicts, self.now, self.taken = listings, verdicts, 0.0, []
        self.clashes: set[frozenset[int]] = set()
        self.fx = argparse.Namespace(
            pulls=self.pulls, issue_labels=dict, clashes=lambda pulls: self.clashes,
            turn=self.turn, head=lambda n: f"pushed{n}",
            clock=lambda: self.now, sleep=self.sleep, say=lambda *a: None,
            memory=Path(tmp, watch.MEMORY), bots=queue.BOTS, merged=queue.MERGED,
            ends={queue.GREEN, queue.DRY}, stops={queue.STOPPED}, skips={queue.NOT_READY},
        )

    def pulls(self):
        listing = self.listings[0] if len(self.listings) == 1 else self.listings.pop(0)
        return [p for p in listing if p["number"] not in self.merged()] if listing is not None else None

    def merged(self):
        return {n for n, s in zip(self.taken, (self.verdicts[n] for n in self.taken)) if s == queue.MERGED}

    def turn(self, number):
        self.taken.append(number)
        return number, self.verdicts[number], "why"

    def sleep(self, seconds):
        self.now += seconds


def opts(*flags, **kw):
    return queue.parse(["--watch", "--no-check", *flags, *(f"--{k}={v}" for k, v in kw.items())])


class Endings(unittest.TestCase):
    def run_world(self, listings, verdicts, *flags, **kw):
        with tempfile.TemporaryDirectory() as tmp:
            world = World(tmp, listings, verdicts)
            results, why, status = watch.run(world.fx, opts(*flags, **kw))
            return world, results, why, status, watch.recall(world.fx.memory)

    def test_merges_each_ready_one_then_ends_when_none_is_open(self):
        world, results, why, status, _ = self.run_world([[pr(2), pr(1)]], {1: queue.MERGED, 2: queue.MERGED})
        self.assertEqual(world.taken, [1, 2])
        self.assertEqual(status, 0)
        self.assertIn("no open pull request", why)

    def test_waits_for_a_draft_to_turn_ready(self):
        listings = [[pr(1, draft=True)], [pr(1, draft=True)], [pr(1)]]
        world, *_ = self.run_world(listings, {1: queue.MERGED})
        self.assertEqual(world.taken, [1])

    def test_a_hand_back_is_remembered_and_the_rest_merge(self):
        world, results, why, status, memory = self.run_world([[pr(1, head="pushed1"), pr(2)]], {1: queue.HANDED_BACK, 2: queue.MERGED}, idle=1, poll=30)
        self.assertEqual(world.taken, [1, 2])
        self.assertEqual(status, watch.HANDED_BACK_STATUS)
        self.assertEqual(memory, {1: "pushed1"})

    def test_a_restart_passes_the_unmoved_hand_back_over(self):
        with tempfile.TemporaryDirectory() as tmp:
            world = World(tmp, [[pr(1, head="pushed1"), pr(2)]], {2: queue.MERGED})
            watch.remember(world.fx.memory, {1: "pushed1"})
            _, why, status = watch.run(world.fx, opts(idle=1, poll=30))
        self.assertEqual(world.taken, [2])
        self.assertEqual(status, 0)
        self.assertIn("#1", why)

    def test_the_machine_failing_stops_without_blaming_the_branch(self):
        world, results, why, status, memory = self.run_world([[pr(1), pr(2)]], {1: queue.STOPPED})
        self.assertEqual(status, watch.MACHINE_STATUS)
        self.assertEqual(memory, {})
        self.assertIn("not the branch", why)

    def test_github_failing_every_listing_stops(self):
        _, _, why, status, _ = self.run_world([None], {})
        self.assertEqual(status, watch.MACHINE_STATUS)
        self.assertIn("listings in a row", why)

    def test_only_idle_drafts_end_the_watch(self):
        world, _, why, status, _ = self.run_world([[pr(1, draft=True)]], {}, idle=5, poll=60)
        self.assertEqual(status, 0)
        self.assertGreaterEqual(world.now, 5 * 60)
        self.assertIn("#1", why)

    def test_a_raced_draft_is_neither_merged_nor_handed_back(self):
        listings = [[pr(1)], []]
        world, _, why, status, memory = self.run_world(listings, {1: queue.NOT_READY})
        self.assertEqual(status, 0)
        self.assertEqual(memory, {})

    def test_a_dry_run_previews_each_ready_one_once(self):
        world, _, why, status, _ = self.run_world([[pr(1), pr(2, draft=True)]], {1: queue.DRY}, "--dry-run")
        self.assertEqual(world.taken, [1])
        self.assertIn("dry run", why)


class Arguments(unittest.TestCase):
    def test_watch_composes_with_no_check(self):
        o = queue.parse(["--watch", "--no-check"])
        self.assertTrue(o.watch and o.no_check)
        self.assertEqual(o.prs, [])

    def test_named_mode_is_unchanged_and_excludes_watch(self):
        self.assertEqual(queue.parse(["524", "526"]).prs, [524, 526])
        for argv in ([], ["1", "--watch"]):
            with self.assertRaises(SystemExit), unittest.mock.patch("sys.stderr"):
                queue.parse(argv)

    def test_a_raced_draft_is_renamed_only_for_the_take_refusals(self):
        self.assertEqual(queue.unready((1, queue.HANDED_BACK, queue.A_DRAFT))[1], queue.NOT_READY)
        self.assertEqual(queue.unready((1, queue.HANDED_BACK, "red CI"))[1], queue.HANDED_BACK)


if __name__ == "__main__":
    unittest.main()
