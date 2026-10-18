#!/usr/bin/env python3
"""#929: the unticked "For a human" items of the PRs merged since a time, gathered.

GitHub is faked by an injected runner that answers `gh api` paths from a table,
so these run offline.
"""

from __future__ import annotations

import contextlib
import io
import json
import subprocess
import unittest
import unittest.mock
from datetime import datetime, timezone

from tests import load

checks = load("human-checks.py")

NOW = datetime(2026, 10, 16, 20, 0, tzinfo=timezone.utc)
BODY = """## What changed
- [ ] not a human check: wrong heading

## For a human
- [ ] Hear the footsteps on real speakers
- [x] Already ticked
  - [ ] A nested one counts too

### Window
- [ ] Resize the window: the HUD stays anchored

## Gates
- [ ] after the section ends
"""


def pull(number: int, merged: str | None, updated: str, body: str = "") -> dict:
    return {"number": number, "title": f"t{number}", "merged_at": merged,
            "updated_at": updated, "body": body}


class FakeGh:
    def __init__(self, pages: list[list[dict]], issues: list[dict] | None = None):
        self.pages, self.issues, self.paths = pages, issues or [], []

    def __call__(self, argv, **_):
        path = argv[2]
        self.paths.append(path)
        if "/pulls?" in path:
            page = int(path.rsplit("page=", 1)[1])
            out = self.pages[page - 1] if page <= len(self.pages) else []
        else:
            out = self.issues
        return subprocess.CompletedProcess(argv, 0, json.dumps(out), "")


def run(arg: str, fake: FakeGh) -> str:
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        checks.main([arg], checks.Gh(fake), repo="o/r", now=NOW)
    return out.getvalue()


class HumanChecks(unittest.TestCase):
    def test_flags_unticked_items_under_for_a_human_and_its_subheadings(self):
        self.assertEqual(checks.human_checks(BODY), [
            "Hear the footsteps on real speakers",
            "A nested one counts too",
            "Resize the window: the HUD stays anchored",
        ])

    def test_passes_a_body_with_no_for_a_human_heading(self):
        self.assertEqual(checks.human_checks("## Gates\n- [ ] CI\n"), [])
        self.assertEqual(checks.human_checks(None), [])

    def test_a_sentence_under_the_heading_is_not_an_item(self):
        self.assertEqual(checks.human_checks("## For a human\nNothing needs one.\n"), [])


class Since(unittest.TestCase):
    def test_reads_ages_dates_and_times(self):
        self.assertEqual(checks.since("12h", NOW), datetime(2026, 10, 16, 8, tzinfo=timezone.utc))
        self.assertEqual(checks.since("2d", NOW), datetime(2026, 10, 14, 20, tzinfo=timezone.utc))
        self.assertEqual(checks.since("2026-10-15", NOW), datetime(2026, 10, 15, tzinfo=timezone.utc))
        self.assertEqual(checks.since("2026-10-15T18:00Z", NOW).hour, 18)

    def test_refuses_what_it_cannot_read(self):
        with self.assertRaises(SystemExit):
            checks.since("yesterday", NOW)


class Main(unittest.TestCase):
    def test_lists_merged_prs_checks_and_double_staged_issues(self):
        first = [pull(9, "2026-10-16T19:00:00Z", "2026-10-16T19:00:00Z", BODY),
                 pull(8, None, "2026-10-16T18:00:00Z", BODY)]  # closed, never merged
        fake = FakeGh([first], issues=[{"number": 5, "title": "t5"},
                                       {"number": 6, "title": "pr", "pull_request": {}}])
        out = run("1d", fake)
        self.assertIn("### #9 t9\n\n- [ ] Hear the footsteps on real speakers", out)
        self.assertNotIn("#8", out)
        self.assertIn("- #5 t5", out)
        self.assertNotIn("#6", out)
        self.assertIn("labels=planning,human", fake.paths[-1])

    def test_stops_paging_at_the_first_pr_updated_before_since(self):
        full = [pull(n, "2026-10-16T19:00:00Z", "2026-10-16T19:00:00Z") for n in range(100)]
        old = [pull(200, "2026-10-01T00:00:00Z", "2026-10-01T00:00:00Z", BODY)]
        fake = FakeGh([full, old, [pull(300, None, "2026-09-01T00:00:00Z")]])
        out = run("1d", fake)
        self.assertEqual(sum("/pulls?" in p for p in fake.paths), 2)
        self.assertNotIn("#200", out)
        self.assertIn("None: 100 PR(s) merged, no unticked human check among them.", out)

    def test_repo_name_from_a_proxy_origin(self):
        def git(argv, **_):
            return subprocess.CompletedProcess(argv, 0, "http://proxy@127.0.0.1:1/git/O/r.git\n", "")
        with unittest.mock.patch.dict("os.environ", {"GITHUB_REPOSITORY": ""}):
            self.assertEqual(checks.repo_name(git), "O/r")

    def test_needs_exactly_one_argument(self):
        with self.assertRaises(SystemExit):
            checks.main([], checks.Gh(FakeGh([])), repo="o/r", now=NOW)


if __name__ == "__main__":
    unittest.main()
