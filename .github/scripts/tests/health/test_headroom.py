"""`ci_headroom.py` tells a job past three quarters of its limit, once (#2276).

A fake `gh` answers the run's jobs and the open issues from canned lists and
records every POST, so nothing reaches GitHub. The case it must flag is a job at
80 % of its limit; the case it must pass, one at 50 %.
"""

from __future__ import annotations

import json
import unittest

import ci_headroom
from tests import SCRIPTS

WORKFLOW = ("jobs:\n  build:\n    timeout-minutes: 20\n    steps:\n      - name: a\n"
            "        timeout-minutes: 2\n  cross:\n    strategy: {}\n    timeout-minutes: 40\n  bare:\n    steps: []\n")
RUN = {"id": 7, "name": "ci", "head_branch": "main", "event": "push", "head_sha": "abc", "html_url": "http://run"}


def job(name: str, minutes: float, conclusion: str = "success") -> dict:
    whole = int(minutes * 60)
    return {"name": name, "conclusion": conclusion, "started_at": "2026-11-16T10:00:00Z",
            "completed_at": f"2026-11-16T{10 + whole // 3600:02}:{whole % 3600 // 60:02}:{whole % 60:02}Z"}


class Fake:
    def __init__(self, jobs: list[dict], issues: list[dict] | None = None, comments: list[dict] | None = None):
        self.jobs, self.issues, self.comments, self.posts = jobs, issues or [], comments or [], []

    def __call__(self, *args: str, stdin: str | None = None) -> object:
        if "POST" in args:
            self.posts.append((args[3], json.loads(stdin)))
            return {}
        path = args[-1]
        if "/jobs?" in path:
            return {"jobs": self.jobs}
        return self.comments if "/comments" in path else self.issues


def report(fake: Fake, run: dict = RUN) -> list[str]:
    return ci_headroom.report(fake, "o/r", run, WORKFLOW)


class Limits(unittest.TestCase):
    def test_a_jobs_own_limit_not_a_steps(self):
        self.assertEqual(ci_headroom.limits(WORKFLOW), {"build": 20, "cross": 40})

    def test_every_ci_job_has_a_limit(self):
        text = (SCRIPTS.parent / "workflows" / "ci.yml").read_text(encoding="utf-8")
        found = ci_headroom.limits(text)
        for name in ("changes", "build-test", "build-test-cross", "blender", "deny", "fuzz-check", "ci-gate"):
            self.assertIn(name, found)


class Headroom(unittest.TestCase):
    def test_flags_a_job_at_80_percent(self):
        fake = Fake([job("build", 16)])
        report(fake)
        [(path, payload)] = fake.posts
        self.assertEqual(path, "repos/o/r/issues")
        self.assertEqual(payload["labels"], ["infrastructure", "agent"])
        self.assertIn(ci_headroom.marker("build"), payload["body"])
        self.assertIn("16.0 min of its 20-minute", payload["body"])

    def test_passes_a_job_at_50_percent(self):
        fake = Fake([job("build", 10), job("cross (macos-26, 1/2)", 20)])
        self.assertIn("within", report(fake)[0])
        self.assertEqual(fake.posts, [])

    def test_a_cancelled_skipped_or_unfinished_job_is_not_measured(self):
        unfinished = {**job("build", 19), "completed_at": None}
        fake = Fake([job("build", 19, "cancelled"), job("cross", 39, "skipped"), unfinished])
        report(fake)
        self.assertEqual(fake.posts, [])

    def test_a_matrix_job_is_told_once_by_its_slowest_entry(self):
        fake = Fake([job("cross (macos-26, 1/2)", 31), job("cross (macos-26, 2/2)", 33), job("bare", 99)])
        report(fake)
        [(_, payload)] = fake.posts
        self.assertIn("`cross (macos-26, 2/2)`, 33.0 min of its 40-minute", payload["body"])

    def test_an_open_issue_gets_a_comment_only_when_the_time_moved(self):
        issue = {"number": 5, "body": f"{ci_headroom.marker('build')}\n<!-- ci-headroom:minutes=16.0 -->"}
        steady = Fake([job("build", 16.5)], [issue])
        report(steady)
        self.assertEqual(steady.posts, [])
        moved = Fake([job("build", 17.5)], [issue], [{"body": "<!-- ci-headroom:minutes=16.4 -->"}])
        report(moved)
        [(path, payload)] = moved.posts
        self.assertEqual(path, "repos/o/r/issues/5/comments")
        self.assertIn("17.5 min", payload["body"])

    def test_dry_run_writes_nothing(self):
        fake = Fake([job("build", 16)])
        said = ci_headroom.report(fake, "o/r", RUN, WORKFLOW, dry_run=True)
        self.assertEqual(fake.posts, [])
        self.assertTrue(said[0].startswith("DRY-RUN: open an issue"))

    def test_only_a_ci_run_on_main_is_measured(self):
        for run in ({**RUN, "name": "lint"}, {**RUN, "head_branch": "x"}, {**RUN, "event": "pull_request"}):
            fake = Fake([job("build", 19)])
            self.assertIn("nothing to do", report(fake, run)[0])
            self.assertEqual(fake.posts, [])


if __name__ == "__main__":
    unittest.main()
