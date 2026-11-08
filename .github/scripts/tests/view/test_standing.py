"""#1876: a coder's *Standing down* comment on a batch issue reaches the stream once."""

from __future__ import annotations

import unittest
from datetime import datetime, timezone
from unittest import mock

from merging.view import standing


def comment(id_, number, body, assoc="OWNER"):
    return {"id": id_, "issue_url": f"https://api.github.com/repos/o/r/issues/{number}", "body": body,
            "author_association": assoc, "user": {"login": "me"}, "html_url": f"https://x/{id_}"}


LISTED = [
    comment(1, 10, "Standing down\n\n#9 is still open."),
    comment(2, 11, "Standing down: blocked by #3"),  # not a batch issue
    comment(3, 10, "Standing down", assoc="NONE"),  # someone who cannot push
    comment(4, 10, "I was not standing down"),
]


class Poll(unittest.TestCase):
    def test_says_each_trusted_stand_down_on_a_batch_issue_once(self):
        asked = []

        def get(path):
            asked.append(path)
            return LISTED if "page=1" in path else []
        with mock.patch("ghrest.slug", return_value="o/r"):
            poll = standing.Poll(get, datetime(2026, 11, 8, 12, 0, tzinfo=timezone.utc), batch=lambda n: n == 10)
            first, second = poll(), poll()
        self.assertEqual(first, ["STANDING DOWN #10 (me): Standing down https://x/1"])
        self.assertEqual(second, [])
        self.assertIn("since=2026-11-08T11:25:00Z", asked[0])  # looks back past the stream's start

    def test_a_batch_issue_has_a_brief_or_an_open_pull_request_closing_it(self):
        def get(path):
            if "page=1" not in path:
                return []
            if path.startswith("repos/o/r/issues/10/"):
                return [comment(5, 10, "Brief\n\nBranch: x")]
            if path.startswith("repos/o/r/pulls"):
                return [{"body": "Does it.\n\nCloses #12\n"}]
            return []
        with mock.patch("ghrest.slug", return_value="o/r"):
            self.assertTrue(standing.in_batch(get, 10))
            self.assertTrue(standing.in_batch(get, 12))
            self.assertFalse(standing.in_batch(get, 13))


if __name__ == "__main__":
    unittest.main()
