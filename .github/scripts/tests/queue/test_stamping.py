#!/usr/bin/env python3
"""#968: a rebase the queue pushes dates the pull request's Gates line.

One line, naming the `main` it rebased onto and the head CI judges, written
once: a second rebase replaces it. A bot's description is left alone, and a
failure to write it never stops the take.
"""

from __future__ import annotations

import json
import subprocess
import unittest
from unittest import mock

from merging.queue import gitio, pulls, stamping
from tests.queue.fixtures import READY, completed
from tests.queue import test_taking

MAIN, NEW = "m" * 40, "n" * 40
BODY = "Closes #1\n\n**Gates**: all green on `79e180a`.\n"


class Line(unittest.TestCase):
    def test_it_names_main_and_the_new_head(self):
        self.assertEqual(stamping.line(MAIN, NEW), "_rebased by the queue onto `mmmmmmm`; local gates not re-run; CI judged `nnnnnnn`._")

    def test_written_once_and_a_second_rebase_replaces_it(self):
        once = stamping.stamped(BODY, stamping.line(MAIN, NEW))
        self.assertTrue(once.startswith(BODY.rstrip()))
        self.assertEqual(stamping.stamped(once, stamping.line(MAIN, NEW)), once)
        twice = stamping.stamped(once, stamping.line("a" * 40, "b" * 40))
        self.assertEqual(twice.count(stamping.PREFIX), 1)
        self.assertIn("`bbbbbbb`", twice)
        self.assertIn("79e180a", twice)

    def test_an_empty_description_gets_just_the_line(self):
        self.assertEqual(stamping.stamped(None, "x").strip(), "x")


class Stamp(unittest.TestCase):
    def stamp(self, pull=READY, current=None, patch=completed(0)):
        current = {"body": BODY} if current is None else current
        with mock.patch.object(gitio, "git", return_value=completed(0)) as git, \
                mock.patch.object(pulls, "quietly", return_value=current), \
                mock.patch.object(subprocess, "run", return_value=patch) as run, \
                mock.patch("builtins.print") as printed:
            git.return_value = subprocess.CompletedProcess([], 0, MAIN + "\n", "")
            stamping.stamp("o/r", pull, NEW, ".")
        return run, printed

    def test_the_description_is_patched_with_the_line(self):
        run, _ = self.stamp()
        argv, sent = run.call_args.args[0], json.loads(run.call_args.kwargs["input"])["body"]
        self.assertEqual(argv[:4], ["gh", "api", "-X", "PATCH"])
        self.assertIn(stamping.line(MAIN, NEW), sent)

    def test_dependabot_is_left_alone(self):
        run, _ = self.stamp(pull={**READY, "author": {"login": "app/dependabot"}})
        run.assert_not_called()

    def test_a_failure_is_said_and_nothing_stops(self):
        run, printed = self.stamp(patch=completed(1, "HTTP 502"))
        self.assertIn("could not date the Gates line", printed.call_args.args[0])


class InTheTake(unittest.TestCase):
    def test_a_pushed_rebase_stamps_once_and_an_unmoved_head_does_not(self):
        taker = test_taking.Taking("test_no_merge_stops_at_green")
        with mock.patch.object(stamping, "stamp") as stamped:
            taker.take(advance=(NEW, []))
            self.assertEqual(stamped.call_count, 1)
            self.assertEqual(stamped.call_args.args[2], NEW)
            stamped.reset_mock()
            taker.take()
            stamped.assert_not_called()


if __name__ == "__main__":
    unittest.main()
