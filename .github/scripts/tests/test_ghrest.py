#!/usr/bin/env python3
"""`ghrest`: the repository from `origin`, explicit pages, and a REST pull
request in the field names the merge path reads (#970)."""

from __future__ import annotations

import unittest
from datetime import datetime

import ghrest
from merging.verdict import github
from merging.watch import board

REST = {
    "number": 7, "title": "T", "body": "Closes #5\nfixes: #6", "state": "open", "merged_at": None,
    "draft": True, "head": {"sha": "abc", "ref": "5-x"}, "base": {"ref": "main"},
    "mergeable": None, "mergeable_state": "dirty", "user": {"login": "dependabot[bot]"},
    "labels": [{"name": "bug", "color": "f00"}], "created_at": "2026-10-17T20:00:00Z",
}


class Slug(unittest.TestCase):
    def test_every_remote_shape_names_the_repository(self):
        for url in ("https://github.com/o/r", "https://github.com/o/r.git\n", "git@github.com:o/r.git",
                    "http://proxy@127.0.0.1:41/git/o/r"):
            self.assertEqual(ghrest.slug_of(url), "o/r", url)

    def test_no_remote_is_none(self):
        self.assertIsNone(ghrest.slug_of(""))


class Pages(unittest.TestCase):
    def test_pages_are_asked_for_explicitly_until_a_short_one(self):
        asked = []
        full = list(range(ghrest.PER_PAGE))

        def get(path):
            asked.append(path)
            return {"items": full if len(asked) == 1 else [1]}
        self.assertEqual(ghrest.pages(get, "repos/o/r/x?state=open", "items"), [*full, 1])
        self.assertEqual(asked[1], "repos/o/r/x?state=open&per_page=100&page=2")

    def test_a_page_that_did_not_come_is_none(self):
        self.assertIsNone(ghrest.pages(lambda _: None, "repos/o/r/issues"))


class Pull(unittest.TestCase):
    def test_rest_fields_take_graphql_names(self):
        pull = ghrest.pull(REST, [{"filename": "a.png", "status": "removed"}], [])
        self.assertEqual(
            {k: pull[k] for k in ("state", "isDraft", "headRefOid", "headRefName", "mergeable", "mergeStateStatus")},
            {"state": "OPEN", "isDraft": True, "headRefOid": "abc", "headRefName": "5-x",
             "mergeable": "UNKNOWN", "mergeStateStatus": "DIRTY"},
        )
        self.assertEqual(pull["author"], {"login": "dependabot[bot]"})
        self.assertEqual(pull["labels"], [{"name": "bug"}])
        # The watch's `--since` drops a pull opened before it (#928); one with no time is kept.
        self.assertEqual(board.recent([pull], datetime.fromisoformat("2026-10-17T21:00:00+00:00")), [])
        self.assertEqual(pull["closingIssuesReferences"], [{"number": 5}, {"number": 6}])
        self.assertEqual(pull["files"], [{"path": "a.png", "status": "removed", "additions": 0, "deletions": 0}])

    def test_merged_and_conflicting_read_as_graphql_did(self):
        pull = ghrest.pull({**REST, "state": "closed", "merged_at": "2026-10-01", "mergeable": False})
        self.assertEqual((pull["state"], pull["mergeable"]), ("MERGED", "CONFLICTING"))
        self.assertNotIn("files", pull)


class Read(unittest.TestCase):
    def test_a_pull_is_read_with_its_files_and_its_checks_by_workflow(self):
        answers = {
            "repos/o/r/pulls/7": REST,
            "repos/o/r/pulls/7/files?per_page=100&page=1": [{"filename": "src/a.rs", "status": "added"}],
            "repos/o/r/commits/abc/check-runs?per_page=100&page=1": {"check_runs": [
                {"name": "lint-gate"}, {"name": "build-test-cross (macos-latest)"}, {"name": "changes"}]},
        }
        pull = github.pull("o/r", 7, fetch=lambda _api, path: answers[path])
        self.assertEqual(github.paths(pull), ["src/a.rs"])
        self.assertEqual(pull["statusCheckRollup"], [
            {"name": "lint-gate", "workflowName": "lint"},
            {"name": "build-test-cross (macos-latest)", "workflowName": "ci"},
        ])


if __name__ == "__main__":
    unittest.main()
