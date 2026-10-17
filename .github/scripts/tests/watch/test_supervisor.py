#!/usr/bin/env python3
"""#908: the supervisor keeps the watch running across its planned exits.

[`supervisor.next_step`] is pure; [`supervisor.run`] is driven with fake
effects; the checkout update and the pass-through of one watch with a real git
repository and a stand-in `merge-queue.py`.
"""

from __future__ import annotations

import argparse
import subprocess
import tempfile
import unittest
import unittest.mock
from pathlib import Path

from merging.watch import daemon, ending, lock, supervisor as sv


class NextStep(unittest.TestCase):
    def test_each_exit_has_its_answer(self):
        self.assertEqual(sv.next_step(0, f"{ending.DEADLINE} (70 minutes); 2 merged", 0)[:2], (sv.AGAIN, 0))
        self.assertEqual(sv.next_step(1, f"{ending.DEADLINE} (70 minutes)", 0)[:2], (sv.AGAIN, 0))
        self.assertEqual(sv.next_step(0, "no open pull request is left.", 0)[:2], (sv.AGAIN, sv.PAUSE_SECONDS))
        self.assertEqual(sv.next_step(ending.RELAUNCH_STATUS, "relaunch: …", 0)[:2], (sv.AGAIN, 0))
        self.assertEqual(sv.next_step(ending.STOPPED_STATUS, "", 0)[0], sv.GIVE_UP)
        self.assertEqual(sv.next_step(lock.LOCKED_STATUS, "", 0)[0], sv.GIVE_UP)
        self.assertEqual(sv.next_step(2, "", 0)[0], sv.GIVE_UP)

    def test_the_machine_failing_backs_off_then_gives_up(self):
        delays = [sv.next_step(ending.MACHINE_STATUS, "", n)[1] for n in range(len(sv.BACKOFF))]
        self.assertEqual(tuple(delays), sv.BACKOFF)
        self.assertEqual(sv.next_step(ending.MACHINE_STATUS, "", len(sv.BACKOFF))[0], sv.GIVE_UP)


def fake(exits, stop_at=None, refused=None, restart=False):
    said, calls = [], {"watch": 0, "slept": 0.0, "stops": 0}

    def stop_requested():
        calls["stops"] += 1
        return stop_at is not None and calls["watch"] >= stop_at

    def watch():
        calls["watch"] += 1
        return exits.pop(0)

    def sleep(s):
        calls["slept"] += s

    def restarted():
        raise SystemExit("restarted")
    fx = argparse.Namespace(
        stop_requested=stop_requested, update=lambda: refused, restart_needed=lambda: restart, restart=restarted,
        watch=watch, sleep=sleep, say=said.append,
    )
    return fx, said, calls


class Run(unittest.TestCase):
    def test_relaunches_through_deadline_hand_back_and_code_change_until_stopped(self):
        exits = [(0, ending.DEADLINE), (1, ending.DEADLINE), (ending.RELAUNCH_STATUS, "relaunch"), (ending.STOPPED_STATUS, "")]
        fx, said, calls = fake(exits)
        self.assertEqual(sv.run(fx), ending.STOPPED_STATUS)
        self.assertEqual((calls["watch"], calls["slept"]), (4, 0))

    def test_nothing_left_pauses_and_a_stop_during_the_pause_ends_it(self):
        fx, said, calls = fake([(0, "no open pull request is left.")], stop_at=1)
        self.assertEqual(sv.run(fx), ending.STOPPED_STATUS)
        self.assertLess(calls["slept"], sv.PAUSE_SECONDS)

    def test_the_machine_retries_a_bounded_number_of_times(self):
        fx, _, calls = fake([(ending.MACHINE_STATUS, "")] * (len(sv.BACKOFF) + 1))
        self.assertEqual(sv.run(fx), ending.MACHINE_STATUS)
        self.assertEqual(calls["slept"], sum(sv.BACKOFF))

    def test_a_dirty_checkout_is_refused_and_its_own_change_restarts_it(self):
        fx, said, _ = fake([], refused="dirty")
        self.assertEqual(sv.run(fx), 1)
        with self.assertRaises(SystemExit):
            sv.run(fake([], restart=True)[0])


class Machine(unittest.TestCase):
    def test_update_detaches_a_clean_checkout_at_origin_main_and_refuses_a_dirty_one(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = lambda *a: subprocess.run(["git", *a], cwd=tmp, capture_output=True, text=True).stdout.strip()  # noqa: E731
            run("init", "-q", "-b", "main"), run("config", "user.email", "t@t"), run("config", "user.name", "t")
            Path(tmp, "a").write_text("1\n")
            run("add", "."), run("commit", "-q", "-m", "one"), run("update-ref", "refs/remotes/origin/main", "main")
            self.assertIsNone(daemon.update(tmp))
            self.assertEqual(run("rev-parse", "HEAD"), run("rev-parse", "origin/main"))
            Path(tmp, "a").write_text("2\n")
            self.assertIn("uncommitted", daemon.update(tmp))

    def test_one_watch_passes_its_output_through_and_reports_how_it_ended(self):
        with tempfile.TemporaryDirectory() as tmp:
            script = Path(tmp, ".github", "scripts", "merge-queue.py")
            script.parent.mkdir(parents=True)
            script.write_text("import sys\nprint('queue: watch ended: relaunch: x', sys.argv[1:])\nsys.exit(5)\n")
            with unittest.mock.patch("builtins.print") as printed:
                status, ended = daemon.watch(tmp, ["--no-check"])
        self.assertEqual(status, 5)
        self.assertTrue(ended.startswith("relaunch: x"))
        self.assertIn("--watch", printed.call_args_list[0].args[0])

    def test_the_command_line_refuses_what_a_supervised_watch_cannot_take(self):
        self.assertEqual(daemon.parse(["--no-check", "--poll", "30"])[1], ["--no-check", "--poll", "30"])
        for bad in (["--dry-run"], ["--for", "70"], ["--watch"]):
            with self.subTest(bad=bad), self.assertRaises(SystemExit), unittest.mock.patch("sys.stderr"):
                daemon.parse(bad)


if __name__ == "__main__":
    unittest.main()
