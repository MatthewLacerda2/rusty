#!/usr/bin/env python3
"""The queue waits, and never mistakes *not yet* for either answer.

It is asked repeatedly of a commit it pushed seconds ago, so an absence of
evidence starts out meaning *GitHub has not caught up* and ends up meaning
scorsese#153. Getting that backwards either refuses every branch it pushes or
merges one nothing compiled. Only a red run is trusted at once. Nothing here
talks to `gh` or `git`: the end-to-end cases patch both out.
"""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

TESTS = Path(__file__).resolve().parent
SCRIPT = TESTS.parent / "merge-queue.py"

_spec = importlib.util.spec_from_file_location("merge_queue", SCRIPT)
queue = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(queue)

_fix = importlib.util.spec_from_file_location("fixtures", TESTS / "test_mergeable.py")
fixtures = importlib.util.module_from_spec(_fix)
_fix.loader.exec_module(fixtures)
run, built, drafted, SHA, CODE = fixtures.run, fixtures.built, fixtures.drafted, fixtures.SHA, fixtures.CODE

READY = {
    **fixtures.READY,
    "headRefName": "486-mergeable-queue",
    "state": "OPEN",
    "title": "Port make mergeable and make queue (#486)",
    "author": {"login": "MatthewLacerda2"},
}


def asked(*runs: dict, waited: float = 0.0, jobs=None, pull=None, files=CODE):
    """[`queue.progress`] over these runs, each a genuine build by default."""
    jobs = jobs if jobs is not None else {r["id"]: built(r["name"]) for r in runs}
    return queue.progress(pull or READY, list(runs), jobs, files, waited)


SPENT = queue.RUN_APPEARS_SECONDS + 1


class Polling(unittest.TestCase):
    def test_a_missing_run_is_not_yet_an_answer_right_after_a_push(self):
        state, lines = asked(waited=1.0)
        self.assertEqual(state, queue.WAIT)
        self.assertIn("nothing has built", " ".join(lines))

    def test_one_workflow_present_and_the_other_not_yet_is_waited_on(self):
        self.assertEqual(asked(run("ci"), waited=1.0)[0], queue.WAIT)

    def test_a_listing_that_shows_only_the_drafts_run_is_absence(self):
        # scorsese watched the runs listing omit a live run beside a skipped one.
        state, _ = asked(run("ci"), run("lint"), jobs={1: drafted("ci"), 2: built("lint")}, waited=1.0)
        self.assertEqual(state, queue.WAIT)

    def test_that_grace_does_not_outlast_the_window(self):
        state, lines = asked(run("ci"), run("lint"), jobs={1: drafted("ci"), 2: built("lint")}, waited=SPENT)
        self.assertEqual(state, queue.STOP)
        self.assertIn("ran a gated job", " ".join(lines))

    def test_a_missing_run_becomes_the_answer_once_the_grace_is_spent(self):
        state, lines = asked(waited=SPENT)
        self.assertEqual(state, queue.STOP)
        self.assertIn("no `ci` run exists", " ".join(lines))

    def test_a_red_run_stops_inside_the_grace(self):
        state, lines = asked(run("ci", conclusion="failure"), waited=0.0)
        self.assertEqual(state, queue.STOP)
        self.assertIn("failure", " ".join(lines))

    def test_a_failure_stops_even_with_a_live_run_beside_it(self):
        state, _ = asked(run("ci", conclusion="failure"), run("lint", status="in_progress", conclusion=None))
        self.assertEqual(state, queue.STOP)

    def test_a_run_in_flight_is_waited_on(self):
        state, lines = asked(run("ci", status="queued", conclusion=None), run("lint"))
        self.assertEqual(state, queue.WAIT)
        self.assertIn("queued", " ".join(lines))

    def test_a_superseded_cancelled_run_does_not_stop_the_queue(self):
        old = run("ci", id=1, conclusion="cancelled", created_at="2026-09-30T00:00:00Z")
        state, _ = asked(old, run("ci", id=3), run("lint"), jobs={1: [], 3: built("ci"), 2: built("lint")})
        self.assertEqual(state, queue.GO)

    def test_a_draft_is_stopped_and_not_waited_on(self):
        self.assertEqual(asked(run("ci"), run("lint"), pull={**READY, "isDraft": True})[0], queue.STOP)

    def test_a_genuine_pass_goes(self):
        state, lines = asked(run("ci"), run("lint"))
        self.assertEqual(state, queue.GO)
        self.assertIn("passed", lines[0])

    def test_the_api_doc_refusal_is_mergeables_and_stops(self):
        # Whatever `judge` refuses, this refuses: one definition of a pass.
        jobs = {1: [fixtures.job("build-test", "success"), fixtures.job("ci-gate", "success")], 2: built("lint")}
        state, lines = asked(run("ci"), run("lint"), jobs=jobs, files=["docs/scripting-api.md"])
        self.assertEqual(state, queue.STOP)
        self.assertIn("#525", " ".join(lines))


class Rebasing(unittest.TestCase):
    def test_a_rebase_that_moves_nothing_is_not_pushed(self):
        self.assertFalse(queue.push_needed(SHA, SHA))
        self.assertTrue(queue.push_needed(SHA, "0" * 40))

    def test_the_pre_push_head_is_github_lagging(self):
        state, lines = queue.head_state(SHA, "1" * 40, SHA, waited=1.0)
        self.assertEqual(state, queue.WAIT)
        self.assertIn("not caught up", " ".join(lines))

    def test_the_pre_push_head_stops_being_an_excuse_after_the_grace(self):
        self.assertEqual(queue.head_state(SHA, "1" * 40, SHA, waited=SPENT)[0], queue.STOP)

    def test_a_third_sha_is_somebody_else_and_stops_immediately(self):
        state, lines = queue.head_state("2" * 40, "1" * 40, SHA, waited=0.0)
        self.assertEqual(state, queue.STOP)
        self.assertIn("Somebody else pushed", " ".join(lines))

    def test_the_watched_head_goes(self):
        self.assertEqual(queue.head_state(SHA, SHA, SHA, 0.0)[0], queue.GO)

    def test_conflicted_paths_are_named(self):
        named = queue.conflicts("src/components/kind.rs\nsrc/app/registry.rs\n\n")
        self.assertEqual(named, ["src/components/kind.rs", "src/app/registry.rs"])


class Queueing(unittest.TestCase):
    def test_order_is_kept_and_repeats_dropped(self):
        self.assertEqual(queue.ordered([486, 488, 486, 489]), [486, 488, 489])

    def test_merged_green_and_dry_are_reported_differently(self):
        lines = queue.summary([(1, queue.MERGED, "a"), (2, queue.GREEN, "b"), (3, queue.DRY, "c")])
        self.assertIn("#1: merged", lines[0])
        self.assertIn("#2: green", lines[1])
        self.assertIn("#3: dry run", lines[2])

    def test_merged_branches_are_named_for_cleanup_rather_than_cleaned(self):
        lines = queue.summary([(486, queue.MERGED, "CI passed")])
        self.assertIn("#486", lines[-1])
        self.assertIn(".claude/worktrees/", lines[-1])

    def test_a_hand_back_has_no_cleanup_note(self):
        self.assertEqual(len(queue.summary([(486, queue.HANDED_BACK, "draft")])), 1)

    def test_the_squash_title_carries_the_pr_number_once(self):
        # House style: `Title (#issue) (#pr)`, and never `(#pr) (#pr)`.
        self.assertEqual(queue.subject({"title": "Fix it (#12)", "number": 30}), "Fix it (#12) (#30)")
        self.assertEqual(queue.subject({"title": "Fix it (#30)", "number": 30}), "Fix it (#30)")

    def test_dependabot_is_recognised_by_either_spelling(self):
        for login in ("app/dependabot", "dependabot[bot]"):
            self.assertTrue(queue.is_bot({"author": {"login": login}}))
        self.assertFalse(queue.is_bot(READY))


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
                self.assertTrue(queue.transport(failure))

    def test_a_reasoned_no_is_a_refusal(self):
        for failure in (
            "HTTP 409: Head branch was modified. Review and try the merge again.",
            "GraphQL: Pull Request is not mergeable (mergePullRequest)",
            "Pull request #502 is not mergeable",
        ):
            with self.subTest(failure=failure):
                self.assertFalse(queue.transport(failure))

    def test_the_record_saying_merged_is_a_merge(self):
        for pull in ({"state": "MERGED"}, {"state": "OPEN", "mergedAt": "2026-09-30T01:00:00Z"}):
            self.assertEqual(queue.settled(pull, 0.0)[0], queue.GO)

    def test_silence_is_waited_on_then_unknown_never_refused(self):
        self.assertEqual(queue.settled(None, 1.0)[0], queue.WAIT)
        state, lines = queue.settled({"state": "OPEN"}, queue.MERGE_SETTLES_SECONDS + 1)
        self.assertEqual(state, queue.STOP)
        self.assertIn("unknown", " ".join(lines))
        self.assertNotIn("refused", " ".join(lines))


def completed(code: int, stderr: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess([], code, "", stderr)


class Taking(unittest.TestCase):
    """[`queue.take`] end to end, with `gh`, `git` and the waiting replaced."""

    def take(self, *argv: str, pull=READY, advance=(SHA, []), merge=completed(0), record=None, judged=(True, ["`ci` passed"])):
        opts = queue.parse(["524", "--poll", "0", *argv])
        with mock.patch.object(queue, "look", return_value=pull), \
            mock.patch.object(queue, "git"), \
            mock.patch.object(queue, "advance", return_value=advance) as rebased, \
            mock.patch.object(queue, "on_tip", return_value=True), \
            mock.patch.object(queue, "wait_for", return_value=(queue.GO, ["CI passed"])), \
            mock.patch.object(queue.mergeable, "evidence", return_value=([], {})), \
            mock.patch.object(queue.mergeable, "judge", return_value=judged), \
            mock.patch.object(queue.subprocess, "run", return_value=merge) as called, \
            mock.patch.object(queue, "merge_record", return_value=record) as asked_again, \
            mock.patch("builtins.print"):
            result = queue.take("o/r", 524, opts)
        return result, rebased, called, asked_again

    def test_a_pass_merges_the_watched_commit_with_the_house_title(self):
        (_, state, _), _, called, _ = self.take()
        self.assertEqual(state, queue.MERGED)
        argv = called.call_args.args[0]
        self.assertEqual(argv[:4], ["gh", "pr", "merge", "524"])
        self.assertIn("--squash", argv)
        self.assertEqual(argv[argv.index("--match-head-commit") + 1], SHA)
        self.assertEqual(argv[argv.index("--subject") + 1], "Port make mergeable and make queue (#486) (#524)")
        self.assertNotIn("--delete-branch", argv)

    def test_a_dry_run_pushes_comments_and_merges_nothing(self):
        (_, state, why), rebased, called, _ = self.take("--dry-run", advance=("1" * 40, []))
        self.assertEqual(state, queue.DRY)
        self.assertEqual(rebased.call_args.kwargs, {"push": False})
        called.assert_not_called()
        self.assertIn("would push 1111111", why)

    def test_a_conflict_is_handed_back_and_nothing_merges(self):
        (_, state, why), _, called, _ = self.take(advance=(None, ["b conflicts with `main`."]))
        self.assertEqual(state, queue.HANDED_BACK)
        self.assertIn("conflicts", why)
        called.assert_not_called()

    def test_a_draft_is_handed_back_before_any_rebase(self):
        (_, state, _), rebased, _, _ = self.take(pull={**READY, "isDraft": True})
        self.assertEqual(state, queue.HANDED_BACK)
        rebased.assert_not_called()

    def test_no_merge_stops_at_green(self):
        (_, state, _), _, called, _ = self.take("--no-merge")
        self.assertEqual(state, queue.GREEN)
        called.assert_not_called()

    def test_a_502_that_merged_is_reported_merged(self):
        (_, state, _), *_ = self.take(merge=completed(1, Merging.BAD_GATEWAY), record={"state": "MERGED"})
        self.assertEqual(state, queue.MERGED)

    def test_a_refusal_is_handed_back_without_asking_again(self):
        (_, state, why), _, _, asked_again = self.take(merge=completed(1, "GraphQL: Pull Request is not mergeable"))
        self.assertEqual(state, queue.HANDED_BACK)
        self.assertIn("refused", why)
        asked_again.assert_not_called()

    def test_dependabot_is_never_force_pushed(self):
        bot = {**READY, "author": {"login": "app/dependabot"}}
        (_, state, _), rebased, _, _ = self.take(pull=bot)
        rebased.assert_not_called()
        self.assertEqual(state, queue.MERGED)

    def test_dependabot_behind_main_is_asked_to_rebase_and_waited_for(self):
        bot = {**READY, "author": {"login": "app/dependabot"}}
        moved = {**bot, "headRefOid": "3" * 40}
        opts = queue.parse(["524", "--poll", "0"])
        with mock.patch.object(queue, "on_tip", side_effect=[False, True]), \
            mock.patch.object(queue, "look", return_value=moved), \
            mock.patch.object(queue.subprocess, "run", return_value=completed(0)) as called, \
            mock.patch.object(queue.time, "sleep"), mock.patch("builtins.print"):
            fresh, _ = queue.bot_head(524, SHA, ".", opts)
        self.assertEqual(fresh, "3" * 40)
        self.assertIn(queue.BOT_REBASE, called.call_args.args[0])

    def test_a_dry_run_on_dependabot_behind_main_says_what_it_would_do(self):
        bot = {**READY, "author": {"login": "app/dependabot"}}
        with mock.patch.object(queue, "bot_head", return_value=(SHA, ["would comment `@dependabot rebase`."])):
            (_, state, why), rebased, called, _ = self.take("--dry-run", pull=bot)
        self.assertEqual(state, queue.DRY)
        self.assertIn("would comment", why)
        self.assertNotIn("already on", why)
        rebased.assert_not_called()
        called.assert_not_called()

    def test_dependabot_in_a_dry_run_is_not_commented_on(self):
        opts = queue.parse(["524", "--dry-run"])
        with mock.patch.object(queue, "on_tip", return_value=False), \
            mock.patch.object(queue.subprocess, "run") as called:
            fresh, notes = queue.bot_head(524, SHA, ".", opts)
        called.assert_not_called()
        self.assertEqual(fresh, SHA)
        self.assertIn("would comment", notes[0])


class Contract(unittest.TestCase):
    def test_it_asks_for_at_least_one_pull_request(self):
        done = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True, check=False)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("usage", done.stderr.lower())

    def test_every_flag_carries_help_text(self):
        done = subprocess.run([sys.executable, str(SCRIPT), "--help"], capture_output=True, text=True, check=False)
        self.assertEqual(done.returncode, 0)
        for flag, described in (
            ("--no-merge", "hand each branch back"),
            ("--dry-run", "merge nothing"),
            ("--deadline", "give up waiting"),
            ("--poll", "ask GitHub"),
            ("--root", "rebase in"),
        ):
            self.assertIn(flag, done.stdout)
            self.assertIn(described, " ".join(done.stdout.split()))


if __name__ == "__main__":
    unittest.main()
