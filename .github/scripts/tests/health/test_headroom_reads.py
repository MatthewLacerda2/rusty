"""`ci_headroom.py` files nothing on a lost read, and fails on no limit (#2276).

A read that does not come (the jobs, the open issues, an issue's comments) ends
the run with "nothing measured" before any write, so an outage never files a
duplicate; a workflow file with no `timeout-minutes` is a broken measure, which
fails the step instead of saying every job is within its limit. The limits are
read from the workflow file at the run's own commit.
"""

from __future__ import annotations

import base64
import io
import os
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest import mock

import ci_headroom
from tests.health.test_headroom import RUN, WORKFLOW, Fake, job, report


class Failing(Fake):
    """A `gh` whose reads of any path holding `lost` answer `None`."""

    def __init__(self, lost: str, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.lost = lost

    def __call__(self, *args: str, stdin: str | None = None) -> object:
        return None if "POST" not in args and self.lost in args[-1] else super().__call__(*args, stdin=stdin)


class LostReads(unittest.TestCase):
    ISSUE = {"number": 5, "body": f"{ci_headroom.marker('build')}\n<!-- ci-headroom:minutes=12.0 -->"}

    def test_a_lost_read_files_nothing(self):
        for lost in ("/jobs?", "issues?state=open", "/comments"):
            with self.subTest(lost=lost):
                fake = Failing(lost, [job("build", 16), job("cross", 39)], [self.ISSUE])
                [said] = report(fake)
                self.assertIn("nothing measured", said)
                self.assertEqual(fake.posts, [])

    def test_no_limit_read_is_a_broken_measure(self):
        for text in ("", "jobs:\n  build:  # a comment\n    timeout-minutes: 20\n"):
            with self.subTest(text=text), self.assertRaises(ci_headroom.NoLimits):
                ci_headroom.report(Fake([job("build", 16)]), "o/r", RUN, text)

    def test_main_fails_the_step_on_no_limit(self):
        fake = Fake([job("build", 16)])

        def gh(*args: str, stdin: str | None = None) -> object:
            return RUN if args[-1] == "repos/o/r/actions/runs/7" else fake(*args, stdin=stdin)

        env = {"REPO": "o/r", "RUN_ID": "7"}
        with mock.patch.dict(os.environ, env), mock.patch.object(ci_headroom, "gh_cli", gh), \
                mock.patch.object(ci_headroom, "workflow_at", lambda *_: ""), \
                redirect_stderr(io.StringIO()) as err, redirect_stdout(io.StringIO()):
            self.assertEqual(ci_headroom.main(), 1)
        self.assertIn("nothing measured", err.getvalue())
        self.assertEqual(fake.posts, [])


class AtTheRunsCommit(unittest.TestCase):
    def test_the_limits_come_from_the_runs_head_sha(self):
        asked = []

        def gh(*args: str, stdin: str | None = None) -> object:
            asked.append(args[-1])
            return {"content": base64.b64encode(WORKFLOW.encode()).decode()}

        run = {**RUN, "path": ".github/workflows/ci.yml@refs/heads/main"}
        self.assertEqual(ci_headroom.workflow_at(gh, "o/r", run), WORKFLOW)
        self.assertEqual(asked, ["repos/o/r/contents/.github/workflows/ci.yml?ref=abc"])

    def test_an_unread_file_is_empty(self):
        self.assertEqual(ci_headroom.workflow_at(lambda *a, **k: None, "o/r", RUN), "")


if __name__ == "__main__":
    unittest.main()
