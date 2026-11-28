"""#2667: `make queue-quiet ARGS=--once`, a cloud fire's chores in one pass, with no supervisor.

Must say: a drain pass that failed, in the one pass a fire runs; a coder's comment posted after the
last fire's poll (and before its lease ended), read by the next fire from `quiet.json`.
Must not: a comment the last fire already said.
"""

from __future__ import annotations

import argparse
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path
from unittest import mock

from merging.view import cli, quiet, standing
from tests.view.test_standing import comment

T0 = datetime(2026, 11, 28, 10, 0, tzinfo=timezone.utc)


def briefed(number, pulls):
    return standing.BRIEFED


class Once(unittest.TestCase):
    def setUp(self):
        made = tempfile.TemporaryDirectory()
        self.addCleanup(made.cleanup)
        self.path = Path(made.name) / quiet.STATE
        self.listed, self.asked = [], []
        slug = mock.patch("ghrest.slug", return_value="o/r")
        slug.start()
        self.addCleanup(slug.stop)

    def get(self, path):
        self.asked.append(path)
        return self.listed if "page=1" in path else []

    def fire(self, at: datetime, since: datetime, failed=()) -> list[str]:
        """One fire: its poll at `at`, its `--since` the lease's end `since`."""
        said = []
        poll = standing.Poll(self.get, since, batch=briefed, clock=lambda: at)
        fx = argparse.Namespace(after_merge=lambda n: [], drain=lambda: list(failed), ready=lambda n: (None, None),
                                bugs=lambda: [], main_runs=lambda: [], comments=poll, say=said.append, path=self.path)
        self.assertEqual(quiet.once(quiet.Quiet(fx)), 0)
        return said

    def test_one_poll_and_one_drain_then_done_without_asking_for_a_supervisor(self):
        made = mock.Mock()
        self.assertEqual(quiet.once(made), 0)
        self.assertEqual([c[0] for c in made.mock_calls], ["poll", "drain"])

    def test_a_failed_drain_is_said_in_the_one_pass(self):
        said = self.fire(T0, T0, failed=["QUIET: `make coders` failed: HTTP 502"])
        self.assertEqual(said, ["QUIET: `make coders` failed: HTTP 502"])

    def test_a_comment_after_the_last_fires_poll_is_the_next_fires(self):
        self.fire(T0 + timedelta(minutes=5), T0)  # fire A polls at 10:05, releases at 10:45
        late = comment(2, 10, "Standing down: #9 first")
        self.listed = [late]
        said = self.fire(T0 + timedelta(minutes=60), T0 + timedelta(minutes=45))
        self.assertEqual(said, ["STANDING DOWN #10 (me): Standing down: #9 first https://x/2"])
        self.assertIn("since=2026-11-28T09:30:00Z", self.asked[-1])  # 10:05 less the look back

    def test_a_comment_the_last_fire_said_is_not_said_again(self):
        self.listed = [comment(1, 10, "Handed back: a decision")]
        self.assertEqual(len(self.fire(T0, T0)), 1)
        self.assertEqual(self.fire(T0 + timedelta(minutes=60), T0 + timedelta(minutes=45)), [])

    def test_the_command_line_takes_the_last_fires_time_for_comments(self):
        with mock.patch.object(cli, "quiet_effects"), mock.patch.object(quiet, "Quiet"), \
                mock.patch.object(quiet, "once", return_value=0) as ran, \
                mock.patch.object(quiet, "follow") as followed, \
                mock.patch.object(cli.standing, "Poll") as poll:
            self.assertEqual(cli.main(["quiet", "--once", "--since", "2026-11-28T05:00:00Z"]), 0)
        ran.assert_called_once()
        followed.assert_not_called()
        self.assertEqual(poll.call_args[0][1].isoformat(), "2026-11-28T05:00:00+00:00")


if __name__ == "__main__":
    unittest.main()
