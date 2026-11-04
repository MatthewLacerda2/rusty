"""#1785's review: the train's effects at their edges.

The squash's "Base branch was modified" retry and `gh`'s cut-off answer, the
wait's runner-outage expiry, the sweep and dispatch with GitHub down, a failed
checkout as a machine stop, `main` read only after a fetch that worked, the
lint tool's data files riding, a half's claims re-asked, and a train as one take.
"""

from __future__ import annotations

import argparse
import subprocess
import unittest
from unittest import mock

from merging.queue import gitio, landing, outcomes, stopping
from merging.train import group, rails, ride, world
from merging.verdict import github
from merging.watch import aging
from tests.train.fakes import Fx, pr
from tests.verdict.fixtures import SHA, built, job, run


def answers(*codes_and_errors):
    calls, script = [], list(codes_and_errors)

    def runner(argv, **kw):
        calls.append(argv)
        code, err = script.pop(0) if len(script) > 1 else script[0]
        return subprocess.CompletedProcess(argv, code, "", err)
    return runner, calls


def squashed(runner):
    with mock.patch.object(landing, "squash_message", return_value=""), mock.patch.object(outcomes, "say"):
        return landing.squash("o/r", {"number": 7, "title": "t", "body": ""}, SHA, ".", 30, runner=runner, sleep=lambda s: None)[0]


class Squash(unittest.TestCase):
    def test_base_branch_modified_is_asked_again_then_goes_back_in_line(self):
        moved = (1, "gh: Base branch was modified. Review and try the merge again. (HTTP 405)")
        runner, calls = answers(moved, (0, ""))
        self.assertEqual((squashed(runner), len(calls)), (outcomes.MERGED, 2))
        runner, calls = answers(moved)
        self.assertEqual((squashed(runner), len(calls)), (landing.BASE_MOVED, 1 + len(landing.BASE_RETRIES)))

    def test_a_cut_off_answer_is_transport_not_a_refusal(self):
        self.assertTrue(landing.transport("unexpected end of JSON input"))


class Effects(unittest.TestCase):
    def test_an_outage_past_the_jobs_grace_is_not_red(self):
        going = {**job("deny", None), "status": "in_progress"}
        jobs = {1: [job("build-test", "cancelled"), going, job("ci-gate", "failure")], 2: built("lint")}
        runs, tip = [run("ci", conclusion="failure"), run("lint")], {"headRefOid": SHA, "isDraft": False}
        self.assertFalse(ride.failed(tip, runs, jobs, lambda r: True))

    def test_the_sweep_and_the_delete_outlive_github_being_down(self):
        down = mock.patch.object(gitio, "remote", side_effect=github.Unreachable("git ls-remote: timeout"))
        with down, mock.patch.object(outcomes, "say") as said:
            self.assertEqual((rails.sweep("."), rails.delete("merge-train/x", ".")), ([], False))
        self.assertIn("none was swept", said.call_args[0][0])

    def test_a_dispatch_retries_a_blip_and_not_a_refusal(self):
        runner, calls = answers((1, "connection reset by peer"), (0, ""))
        self.assertIsNone(rails.dispatch("o/r", "merge-train/x", runner=runner, sleep=lambda s: None))
        self.assertEqual(len(calls), 3)  # lint twice, then ci
        runner, calls = answers((1, "HTTP 422"))
        self.assertIn("lint.yml", rails.dispatch("o/r", "merge-train/x", runner=runner, sleep=lambda s: None))
        self.assertEqual(len(calls), 1)  # nothing dispatched after it

    def test_a_failed_checkout_is_the_machines_failure(self):
        opts = argparse.Namespace(no_check=False, dry_run=False, root=".")
        failed = subprocess.CompletedProcess([], 128, "", "No space left on device")
        with mock.patch.object(gitio, "git", return_value=failed), self.assertRaises(stopping.Stopped):
            world.compiles(SHA, opts)

    def test_a_gh_refusal_reading_own_ci_is_not_green(self):
        for down in (github.Unreachable("HTTP 500"), SystemExit(1)):
            with self.subTest(down=down), mock.patch.object(github, "evidence", side_effect=down):
                self.assertFalse(world.own_green("o/r", 1, SHA))

    def test_main_is_unknown_when_the_fetch_failed(self):
        git = lambda *a: (0, "tree\n") if a[0] == "rev-parse" else (0, "0\n")
        for fetch in (subprocess.CompletedProcess([], 1, "", "fatal"), github.Unreachable("git fetch: timeout")):
            kind = "side_effect" if isinstance(fetch, Exception) else "return_value"
            with mock.patch.object(gitio, "remote", **{kind: fetch}):
                self.assertIsNone(world.main_state(git, ".", "base"))
                self.assertIsNone(world.started(git, "."))
        with mock.patch.object(gitio, "remote", return_value=subprocess.CompletedProcess([], 0, "", "")):
            self.assertEqual(world.main_state(git, ".", "base"), ("tree", 0))


class Riders(unittest.TestCase):
    def test_the_lint_tools_data_rides_and_its_code_goes_alone(self):
        self.assertIsNone(group.alone(["tools/lint/src/layers/table.rs", "tools/lint/src/components/waivers.rs"]))
        self.assertIn("tools/lint/src/main.rs", group.alone(["tools/lint/src/main.rs"]))

    def test_markdown_only_goes_alone_as_cis_code_filter_counts_it(self):
        self.assertIn("Markdown-only", group.alone(["README.md", "docs/testing.md"]))
        for rides in (["docs/api/Transform.md"], ["docs/game-agent.md"], ["README.md", "src/a.rs"]):
            with self.subTest(files=rides):
                self.assertIsNone(group.alone(rides))

    def test_a_machine_stop_keeps_its_whole_diagnosis(self):
        fx = Fx([pr(1), pr(2)])
        fx.ns.check = lambda sha: (_ for _ in ()).throw(stopping.Stopped(["disk full", "cargo: tail", "free space"]))
        yard = fx.yard()
        results, _ = yard.ride(yard.form([pr(1), pr(2)])[1])
        self.assertEqual(results, [(1, outcomes.STOPPED, "disk full")])
        said = []
        fx.ns.say = lambda *a: said.append(a)
        yard.ride(yard.form([pr(1), pr(2)])[1])
        self.assertEqual(said[-1][1:], ("cargo: tail", "free space"))

    def test_a_half_is_asked_its_claims_again(self):
        line = [pr(1), pr(2), pr(3), pr(4)]
        fx = Fx(line, verdicts=(ride.RED,))
        yard = fx.yard()
        yard.ride(yard.form(line)[1])
        fx.ns.claims = lambda n: (n, outcomes.HANDED_BACK, "a stray keyword.") if n == 2 else None
        members, train, notes = yard.form(line)
        self.assertEqual(([p["number"] for p in members], train), ([1], None))
        self.assertIn("#2 waits for a train of its own: a stray keyword", notes[0])

    def test_a_train_is_one_take(self):
        line = aging.Line()
        line.see([1, 2, 3, 4, 5], 0.0)
        line.took(1, 2, 3)
        self.assertEqual(line.passes, {4: 1, 5: 1})


if __name__ == "__main__":
    unittest.main()
