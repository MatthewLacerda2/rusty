#!/usr/bin/env python3
"""#2628: the go-ahead asked again before a merge, on the path the watch takes.

The listing reads a ready, labelled pull request with nothing owed; during its
CI wait a `Needs:` is posted. Must: not merge it when the re-read sees the
`Needs:`, nor when the re-read fails (it goes back in line, read again on the
next pass). Must merge: the re-read succeeding with nothing owed.
"""

from __future__ import annotations

import unittest
from unittest import mock

from merging.queue import cli, gitio, landing, outcomes, pulls, taking, update, verdicts, waiting
from merging.verdict import reviewed
from tests.queue.fixtures import READY, REVIEW
from tests.queue.test_verdicts import Fetch, comment
from tests.verdict.fixtures import SHA

RECORD = "Independent review\nNo findings."
PULL = {**READY, "baseRefName": "main", "updatedAt": "t1", "labels": [{"name": "queue"}]}


def taken(during_wait) -> tuple[str, str, bool]:
    """`take` under the watch, after the listing read `PULL`; `during_wait(fetch)`
    is what happens on GitHub while its CI runs."""
    fetch = Fetch({524: [comment(RECORD)]})
    reader = verdicts.Reader("o/r", fetch, say=lambda line: None)
    listed = reader.attach([PULL])[0]
    assert not listed["standing"]["owed"]  # nothing owed at the listing

    def wait(*args, **kwargs):
        during_wait(fetch)
        return outcomes.GO, ["ci passed"]

    opts = cli.parse(["524", "--poll", "0"])
    opts.watch = True
    with mock.patch.dict(pulls.READERS, {"o/r": reader}), \
        mock.patch("ghrest.slug", return_value="o/r"), \
        mock.patch.object(pulls, "look", return_value={**PULL, "updatedAt": "t2"}), \
        mock.patch.object(reviewed, "comments", return_value=REVIEW), \
        mock.patch.object(gitio, "git"), mock.patch.object(gitio, "remote"), \
        mock.patch.object(update, "advance", return_value=(SHA, [])), \
        mock.patch.object(waiting, "wait_for", side_effect=wait), \
        mock.patch.object(landing, "squash", return_value=(outcomes.MERGED, "")) as squash, \
        mock.patch("builtins.print"):
        _, state, why = taking.take("o/r", 524, opts)
    return state, why, squash.called


def needs(fetch: Fetch) -> None:
    fetch.comments[524].append(comment("Independent review (second read)\nNeeds: a desktop measurement"))


class Reread(unittest.TestCase):
    def test_a_needs_posted_during_the_wait_then_an_unread_re_read_does_not_merge(self):
        def blip(fetch):
            needs(fetch)
            fetch.down = True
        state, why, merged = taken(blip)
        self.assertEqual((state, merged), (outcomes.REQUEUED, False))
        self.assertIn("did not show its comments", why)

    def test_a_needs_the_re_read_sees_does_not_merge(self):
        state, why, merged = taken(needs)
        self.assertEqual((state, merged), (outcomes.REQUEUED, False))
        self.assertIn("lost its go-ahead", why)

    def test_nothing_owed_on_the_re_read_merges(self):
        self.assertEqual(taken(lambda fetch: None)[::2], (outcomes.MERGED, True))


if __name__ == "__main__":
    unittest.main()
