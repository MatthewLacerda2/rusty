#!/usr/bin/env python3
"""#1764: the queue refuses a pull request no comment records a review decision on.

Must refuse: a code pull request with no such comment, one whose comment is by an
account that cannot push, and a `No review:` with no reason. Must pass: an
`Independent review` comment, a reasoned `No review:`, a Markdown-only pull
request CI counts as non-code, and Dependabot's cargo bump.
"""

from __future__ import annotations

import unittest

from merging.verdict import changes, reviewed


def pull(*paths: str, branch: str = "1764-x") -> dict:
    return {"headRefName": branch, "files": [{"path": p} for p in paths]}


def said(body: str, who: str = "OWNER") -> dict:
    return {"body": body, "author_association": who, "user": {"login": "someone"}}


CODE = pull("src/main.rs")


class Refuses(unittest.TestCase):
    def test_a_code_pull_request_with_no_decision(self):
        refused = reviewed.refusal(CODE, [said("LGTM"), said("Brief\nIndependent review later")])
        self.assertIn("review decision", refused[0])

    def test_a_decision_by_someone_who_cannot_push(self):
        for who in ("NONE", "CONTRIBUTOR", "FIRST_TIME_CONTRIBUTOR", None):
            with self.subTest(who=who):
                refused = reviewed.refusal(CODE, [said("Independent review\nNo findings.", who)])
                self.assertTrue(refused)
                self.assertIn("cannot push", refused[1])

    def test_a_record_still_to_come(self):
        for body in ("Independent review pending", "Independent reviewer dispatched", "**Independent review** soon"):
            with self.subTest(body=body):
                self.assertTrue(reviewed.refusal(CODE, [said(body)]))

    def test_a_waiver_with_no_reason(self):
        for body in ("No review:", "No review:   ", "**No review:**", "**No review**:", "No review:\nbecause docs"):
            with self.subTest(body=body):
                self.assertTrue(reviewed.refusal(CODE, [said(body)]))

    def test_a_doc_the_tests_parse_is_code(self):
        for prefix in changes.CODE_DOCS:
            with self.subTest(prefix=prefix):
                self.assertTrue(reviewed.refusal(pull(changes.code_doc_example(prefix)), []))

    def test_a_bot_author_on_a_persons_branch_or_a_person_on_a_bot_branch(self):
        self.assertTrue(reviewed.refusal(pull("Cargo.lock"), [], bot=True))
        self.assertTrue(reviewed.refusal(pull("Cargo.lock", branch="dependabot/cargo/x"), [], bot=False))

    def test_dependabots_actions_bump_edits_workflows(self):
        bump = pull(".github/workflows/ci.yml", branch="dependabot/github_actions/actions/checkout-5")
        self.assertTrue(reviewed.refusal(bump, [], bot=True))


class Passes(unittest.TestCase):
    def test_an_independent_review_record(self):
        for body in ("Independent review", "## Independent review\n1. fixed", "**Independent review** (high): none",
                     "Independent review (high: physics)", "**Independent review**: no findings",
                     "Independent review — none", "\n\nIndependent review: #1753"):
            with self.subTest(body=body):
                self.assertEqual(reviewed.refusal(CODE, [said(body)]), [])

    def test_a_reasoned_waiver(self):
        for body in ("No review: data only", "**No review:** a dev tool", "**No review**: a dev tool",
                     "no review: size-cap split"):
            with self.subTest(body=body):
                self.assertEqual(reviewed.refusal(CODE, [said(body, "COLLABORATOR")]), [])

    def test_a_markdown_only_pull_request(self):
        self.assertEqual(reviewed.refusal(pull("README.md", "docs/testing.md"), []), [])

    def test_dependabots_own_branch(self):
        self.assertEqual(reviewed.refusal(pull("Cargo.lock", branch="dependabot/cargo/rustls-0.24"), [], bot=True), [])


class Reads(unittest.TestCase):
    def test_comments_are_paged_over_rest(self):
        asked = []

        def fetch(*args):
            asked.append(args)
            return [said("x")]
        self.assertEqual(len(reviewed.comments("o/r", 7, fetch)), 1)
        self.assertEqual(asked, [("api", "repos/o/r/issues/7/comments?per_page=100&page=1")])


if __name__ == "__main__":
    unittest.main()
