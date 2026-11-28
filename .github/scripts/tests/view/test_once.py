"""#2667: `make queue-quiet ARGS=--once`, a cloud fire's chores in one pass, with no supervisor."""

from __future__ import annotations

import unittest
from unittest import mock

from merging.view import cli, quiet


class Once(unittest.TestCase):
    def test_one_poll_and_one_drain_then_done_without_asking_for_a_supervisor(self):
        made = mock.Mock()
        self.assertEqual(quiet.once(made), 0)
        self.assertEqual([c[0] for c in made.mock_calls], ["poll", "drain"])

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
