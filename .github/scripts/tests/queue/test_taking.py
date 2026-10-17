#!/usr/bin/env python3
"""[`taking.take`] end to end, and how a failed merge call is read.

Nothing here talks to `gh` or `git`: both are patched out, where they are
defined, so every caller sees the patch.
"""

from __future__ import annotations

import subprocess
import time
import unittest
from unittest import mock

from merging.queue import cli, dependabot, gitio, landing, outcomes, pulls, rebase, taking, waiting
from merging.verdict import decide, github
from tests.queue.fixtures import READY, completed
from tests.verdict.fixtures import SHA


class Merging(unittest.TestCase):
    BAD_GATEWAY = 'non-200 OK status code: 502 Bad Gateway body: "<html>"'

    def test_a_5xx_or_network_error_is_transport(self):
        for failure in (
            self.BAD_GATEWAY,
            "HTTP 503: Service Unavailable",
            "net/http: TLS handshake timeout",
            "read: connection reset by peer",
        ):
            with self.subTest(failure=failure):
                self.assertTrue(landing.transport(failure))

    def test_a_reasoned_no_is_a_refusal(self):
        for failure in (
            "HTTP 409: Head branch was modified. Review and try the merge again.",
            "GraphQL: Pull Request is not mergeable (mergePullRequest)",
            "Pull request #502 is not mergeable",
        ):
            with self.subTest(failure=failure):
                self.assertFalse(landing.transport(failure))

    def test_the_record_saying_merged_is_a_merge(self):
        for pull in ({"state": "MERGED"}, {"state": "OPEN", "mergedAt": "2026-09-30T01:00:00Z"}):
            self.assertEqual(landing.settled(pull, 0.0)[0], outcomes.GO)

    def test_silence_is_waited_on_then_unknown_never_refused(self):
        self.assertEqual(landing.settled(None, 1.0)[0], outcomes.WAIT)
        state, lines = landing.settled({"state": "OPEN"}, landing.MERGE_SETTLES_SECONDS + 1)
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("unknown", " ".join(lines))
        self.assertNotIn("refused", " ".join(lines))


class Taking(unittest.TestCase):
    """[`taking.take`] end to end, with `gh`, `git` and the waiting replaced."""

    def take(self, *argv: str, pull=READY, advance=(SHA, []), merge=completed(0), record=None, judged=(True, ["`ci` passed"])):
        opts = cli.parse(["524", "--poll", "0", *argv])
        with mock.patch.object(pulls, "look", return_value=pull), \
            mock.patch.object(gitio, "git"), \
            mock.patch.object(rebase, "advance", return_value=advance) as rebased, \
            mock.patch.object(rebase, "on_tip", return_value=True), \
            mock.patch.object(waiting, "wait_for", return_value=(outcomes.GO, ["CI passed"])), \
            mock.patch.object(github, "evidence", return_value=([], {})), \
            mock.patch.object(decide, "judge", return_value=judged), \
            mock.patch.object(subprocess, "run", return_value=merge) as called, \
            mock.patch.object(landing, "merge_record", return_value=record) as asked_again, \
            mock.patch("builtins.print"):
            result = taking.take("o/r", 524, opts)
        return result, rebased, called, asked_again

    def test_a_pass_merges_the_watched_commit_with_the_house_title(self):
        (_, state, _), _, called, _ = self.take()
        self.assertEqual(state, outcomes.MERGED)
        argv = called.call_args.args[0]
        self.assertEqual(argv[:5], ["gh", "api", "--method", "PUT", "repos/o/r/pulls/524/merge"])
        self.assertIn("merge_method=squash", argv)
        self.assertIn(f"sha={SHA}", argv)
        self.assertIn("commit_title=Port make mergeable and make queue (#486) (#524)", argv)

    def test_a_dry_run_pushes_comments_and_merges_nothing(self):
        (_, state, why), rebased, called, _ = self.take("--dry-run", advance=("1" * 40, []))
        self.assertEqual(state, outcomes.DRY)
        self.assertEqual(rebased.call_args.kwargs, {"push": False, "check": None})
        called.assert_not_called()
        self.assertIn("would push 1111111", why)

    def test_a_conflict_is_handed_back_and_nothing_merges(self):
        (_, state, why), _, called, _ = self.take(advance=(None, ["b conflicts with `main`."]))
        self.assertEqual(state, outcomes.HANDED_BACK)
        self.assertIn("conflicts", why)
        called.assert_not_called()

    def test_a_draft_is_handed_back_before_any_rebase(self):
        (_, state, _), rebased, _, _ = self.take(pull={**READY, "isDraft": True})
        self.assertEqual(state, outcomes.HANDED_BACK)
        rebased.assert_not_called()

    def test_no_merge_stops_at_green(self):
        (_, state, _), _, called, _ = self.take("--no-merge")
        self.assertEqual(state, outcomes.GREEN)
        called.assert_not_called()

    def test_a_502_that_merged_is_reported_merged(self):
        (_, state, _), *_ = self.take(merge=completed(1, Merging.BAD_GATEWAY), record={"state": "MERGED"})
        self.assertEqual(state, outcomes.MERGED)

    def test_a_refusal_is_handed_back_without_asking_again(self):
        (_, state, why), _, _, asked_again = self.take(merge=completed(1, "GraphQL: Pull Request is not mergeable"))
        self.assertEqual(state, outcomes.HANDED_BACK)
        self.assertIn("refused", why)
        asked_again.assert_not_called()

    def test_dependabot_is_never_force_pushed(self):
        bot = {**READY, "author": {"login": "app/dependabot"}}
        (_, state, _), rebased, _, _ = self.take(pull=bot)
        rebased.assert_not_called()
        self.assertEqual(state, outcomes.MERGED)

    def test_dependabot_behind_main_is_asked_to_rebase_and_waited_for(self):
        bot = {**READY, "author": {"login": "app/dependabot"}}
        moved = {**bot, "headRefOid": "3" * 40}
        opts = cli.parse(["524", "--poll", "0"])
        with mock.patch.object(rebase, "on_tip", side_effect=[False, True]), \
            mock.patch.object(rebase, "main_moved_by_docs", return_value=False), \
            mock.patch.object(pulls, "look", return_value=moved), \
            mock.patch.object(subprocess, "run", return_value=completed(0)) as called, \
            mock.patch.object(time, "sleep"), mock.patch("builtins.print"):
            fresh, _ = dependabot.bot_head(524, SHA, ".", opts)
        self.assertEqual(fresh, "3" * 40)
        self.assertIn(f"body={dependabot.BOT_REBASE}", called.call_args.args[0])

    def test_a_dry_run_on_dependabot_behind_main_says_what_it_would_do(self):
        bot = {**READY, "author": {"login": "app/dependabot"}}
        with mock.patch.object(dependabot, "bot_head", return_value=(SHA, ["would comment `@dependabot rebase`."])):
            (_, state, why), rebased, called, _ = self.take("--dry-run", pull=bot)
        self.assertEqual(state, outcomes.DRY)
        self.assertIn("would comment", why)
        self.assertNotIn("already on", why)
        rebased.assert_not_called()
        called.assert_not_called()

    def test_dependabot_in_a_dry_run_is_not_commented_on(self):
        opts = cli.parse(["524", "--dry-run"])
        with mock.patch.object(rebase, "on_tip", return_value=False), \
            mock.patch.object(rebase, "main_moved_by_docs", return_value=False), \
            mock.patch.object(subprocess, "run") as called:
            fresh, notes = dependabot.bot_head(524, SHA, ".", opts)
        called.assert_not_called()
        self.assertEqual(fresh, SHA)
        self.assertIn("would comment", notes[0])


if __name__ == "__main__":
    unittest.main()
