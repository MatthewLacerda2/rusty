"""#2136: every coder comment on a briefed batch issue reaches the stream, whatever its first line."""

from __future__ import annotations

import unittest
from datetime import datetime, timezone
from unittest import mock

from merging.view import standing
from tests.view.test_standing import comment

SINCE = datetime(2026, 11, 8, 12, 0, tzinfo=timezone.utc)


def run(listed, batch):
    def get(path):
        return listed if "page=1" in path else []
    with mock.patch("ghrest.slug", return_value="o/r"):
        return standing.Poll(get, SINCE, batch=batch)()


def briefed(number, pulls):
    return {10: standing.BRIEFED, 12: standing.CLOSED_BY_PULL, 14: standing.BRIEFED_CLOSED}.get(number, False)


class Comments(unittest.TestCase):
    def test_a_hand_back_with_any_first_line_passes(self):
        said = run([comment(1, 10, "Handed back on draft PR #2104: the bake needs a decision")], briefed)
        self.assertEqual(said, ["COMMENT #10 (me): Handed back on draft PR #2104: the bake needs a decision https://x/1"])

    def test_a_stand_down_keeps_its_own_label(self):
        said = run([comment(1, 10, "**Standing down**: #9 first")], briefed)
        self.assertEqual(said, ["STANDING DOWN #10 (me): **Standing down**: #9 first https://x/1"])

    def test_a_brief_does_not_pass(self):
        self.assertEqual(run([comment(1, 10, "Brief\n\nBranch: x"), comment(2, 10, "## Brief (re-brief)")], briefed), [])

    def test_the_orchestrators_own_note_does_not_pass(self):
        self.assertEqual(run([comment(1, 10, "Context update\n\n#2088 merged."),
                              comment(2, 10, "**Context update:** main moved")], briefed), [])

    def test_a_comment_on_an_issue_with_no_brief_does_not_pass(self):
        listed = [comment(1, 11, "Some discussion"), comment(2, 12, "A note on the PR's issue")]
        self.assertEqual(run(listed, briefed), [])
        # A stand-down still passes on an issue an open pull request closes, as before.
        self.assertEqual(run([comment(3, 12, "Standing down")], briefed), ["STANDING DOWN #12 (me): Standing down https://x/3"])

    def test_someone_who_cannot_push_does_not_pass(self):
        self.assertEqual(run([comment(1, 10, "Handed back", assoc="NONE")], briefed), [])

    def test_an_issue_is_asked_once_per_poll(self):
        asked = []

        def batch(number, pulls):
            asked.append(number)
            return standing.BRIEFED
        said = run([comment(1, 10, "One"), comment(2, 10, "Two"), comment(3, 10, "Brief")], batch)
        self.assertEqual(len(said), 2)
        self.assertEqual(asked, [10])

    def test_a_closed_issue_takes_only_a_stand_down(self):
        self.assertEqual(run([comment(1, 14, "A note weeks later")], briefed), [])
        self.assertEqual(run([comment(2, 14, "Standing down")], briefed), ["STANDING DOWN #14 (me): Standing down https://x/2"])

    def test_a_pull_request_comment_asks_github_nothing(self):
        asked = []
        pr = dict(comment(1, 20, "Independent review"), html_url="https://github.com/o/r/pull/20#issuecomment-1")

        def batch(number, pulls):
            asked.append(number)
            return standing.BRIEFED
        self.assertEqual(run([pr], batch), [])
        self.assertEqual(asked, [])

    def test_two_issues_in_one_poll_read_the_open_pulls_once(self):
        asked = []

        def get(path):
            asked.append(path)
            if "page=1" not in path:
                return []
            if "issues/comments?" in path:
                return [comment(1, 12, "Standing down"), comment(2, 13, "Standing down")]
            if path.startswith("repos/o/r/pulls"):
                return [{"body": "Does it.\n\nCloses #12\n"}]
            return []
        with mock.patch("ghrest.slug", return_value="o/r"):
            said = standing.Poll(get, SINCE)()
        self.assertEqual(said, ["STANDING DOWN #12 (me): Standing down https://x/1"])
        self.assertEqual(sum(p.startswith("repos/o/r/pulls") for p in asked), 1)

    def test_in_batch_says_how_an_issue_is_the_batchs(self):
        def get(path):
            if path == "repos/o/r/issues/10":
                return {"state": "open"}
            if path == "repos/o/r/issues/14":
                return {"state": "closed"}
            if "page=1" not in path:
                return []
            if path.startswith(("repos/o/r/issues/10/", "repos/o/r/issues/14/")):
                return [comment(5, 10, "Brief\n\nBranch: x")]
            return []
        pulls = lambda: [{"body": "Does it.\n\nCloses #12\n"}]  # noqa: E731
        with mock.patch("ghrest.slug", return_value="o/r"):
            self.assertEqual(standing.in_batch(get, 10, pulls), standing.BRIEFED)
            self.assertEqual(standing.in_batch(get, 14, pulls), standing.BRIEFED_CLOSED)
            self.assertEqual(standing.in_batch(get, 12, pulls), standing.CLOSED_BY_PULL)
            self.assertFalse(standing.in_batch(get, 13, pulls))
            self.assertIsNone(standing.in_batch(get, 13, lambda: None))  # the pulls page did not come

if __name__ == "__main__":
    unittest.main()
