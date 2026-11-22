#!/usr/bin/env python3
"""#2628: the listing's read of each ready pull request's standing, kept between polls.

Must: attach a standing to a ready pull request based on `main`, ask GitHub's
compare only for a verdict given on an older head, keep the comments while
`updatedAt` stands and re-read them when it moves, fail the read when the
comments fail, keep what a last read owed when a re-read fails. Must not: fail
the listing on a compare it cannot read (that one is not queued by its verdict,
and named once), nor read a draft or a pull request based elsewhere.
"""

from __future__ import annotations

import unittest

from merging.queue import verdicts

REPO = "o/r"
REVIEW = "Independent review\nNo findings.\nVerdict: queue on abc1234"


class Fetch:
    def __init__(self, comments: dict[int, list[dict]], compare: dict | None = None):
        self.comments, self.compare, self.asked, self.down = comments, compare, [], False

    def __call__(self, path: str):
        self.asked.append(path)
        if self.down:
            return None
        if "/compare/" in path:
            return self.compare
        number = int(path.split("/issues/")[1].split("/")[0])
        return self.comments.get(number, []) if "page=1" in path else []


def comment(body: str) -> dict:
    return {"body": body, "author_association": "OWNER", "created_at": "2026-11-22T10:00:00Z"}


def pull(number: int, head: str = "abc1234" + "0" * 33, updated: str = "t1", **more) -> dict:
    return {"number": number, "isDraft": False, "baseRefName": "main", "headRefOid": head, "updatedAt": updated, **more}


class Reads(unittest.TestCase):
    def test_a_verdict_on_its_own_head_needs_no_compare(self):
        fetch = Fetch({1: [comment(REVIEW)]})
        shaped = verdicts.Reader(REPO, fetch).attach([pull(1)])
        self.assertEqual(shaped[0]["standing"]["verdict"], "queue")
        self.assertFalse([p for p in fetch.asked if "/compare/" in p])

    def test_an_older_head_asks_the_compare_once(self):
        compare = {"ahead_by": 2, "commits": [{"sha": "abc1234", "parents": [{"sha": "a0"}]},
                                              {"sha": "f" * 40, "parents": [{"sha": "abc1234"}]}]}
        fetch = Fetch({1: [comment(REVIEW)]}, compare)
        reader = verdicts.Reader(REPO, fetch)
        for _ in range(2):
            self.assertEqual(reader.attach([pull(1, head="f" * 40)])[0]["standing"]["verdict"], "round")
        self.assertEqual(len([p for p in fetch.asked if "/compare/" in p]), 1)

    def test_drafts_and_other_bases_are_not_read(self):
        fetch = Fetch({})
        shaped = verdicts.Reader(REPO, fetch).attach([pull(1, isDraft=True), pull(2, baseRefName="train")])
        self.assertFalse(fetch.asked)
        self.assertFalse(any("standing" in p for p in shaped))

    def test_comments_kept_until_the_pull_request_moves_or_ages(self):
        now = [0.0]
        fetch = Fetch({1: []})
        reader = verdicts.Reader(REPO, fetch, clock=lambda: now[0])
        reader.attach([pull(1)])
        reader.attach([pull(1)])
        self.assertEqual(len(fetch.asked), 1)
        fetch.comments[1] = [comment(REVIEW)]
        self.assertEqual(reader.attach([pull(1, updated="t2")])[0]["standing"]["verdict"], "queue")
        now[0] = verdicts.FRESH_SECONDS + 1
        reader.attach([pull(1, updated="t2")])
        self.assertEqual(len(fetch.asked), 3)

    def test_github_failing_fails_the_read(self):
        fetch = Fetch({1: [comment(REVIEW)]})
        fetch.down = True
        self.assertIsNone(verdicts.Reader(REPO, fetch, say=lambda line: None).attach([pull(1)]))

    def test_a_compare_unread_is_not_queued_and_named_once(self):
        said = []
        fetch = Fetch({1: [comment(REVIEW)], 2: []}, compare=None)
        reader = verdicts.Reader(REPO, fetch, say=said.append)
        for _ in range(2):
            shaped = reader.attach([pull(1, head="f" * 40), pull(2)])
            self.assertEqual([p["standing"]["verdict"] for p in shaped], ["round", None])
            self.assertIsNone(shaped[0]["standing"]["current"])
        self.assertEqual(len(said), 1)
        self.assertIn("#1", said[0])
        self.assertEqual(len([p for p in fetch.asked if "/compare/" in p]), 2)  # asked again, not kept

    def test_a_failed_re_read_keeps_what_the_last_read_owed(self):
        owing = comment(REVIEW + "\nNeeds: a desktop look")
        fetch = Fetch({1: [owing], 2: [comment(REVIEW)]})
        reader = verdicts.Reader(REPO, fetch, say=lambda line: None)
        reader.attach([pull(1), pull(2)])
        fetch.down = True
        again = reader.again(pull(1, updated="t2", labels=[{"name": "queue"}]))
        self.assertEqual(again["standing"]["owed"], ["a desktop look"])
        self.assertNotIn("standing", reader.again(pull(2, updated="t2")))  # nothing owed: labels alone


if __name__ == "__main__":
    unittest.main()
