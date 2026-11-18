"""`proved_tree.verdict` on a ready PR's run (#2333): the test merge's tree, a
dispatch or push run as its only proof, and never the superseded rule.

`SHA` plays the test merge commit; its first parent is the base the key check
compares against. Every doubt must run the gates in full.
"""

from __future__ import annotations

import unittest

import proved_tree
from tests.proved.test_proved_tree import REPO, SHA, TREE, Fake, built_jobs, job

BASE, HEAD = "a" * 40, "h" * 40


def pr_fake(event: str | None = "workflow_dispatch", parents=(BASE, HEAD), **kw) -> "Asking":
    fake = Fake(**kw)
    fake.answers[f"repos/{REPO}/commits/{SHA}"] = {
        "commit": {"tree": {"sha": TREE}}, "parents": [{"sha": p} for p in parents]}
    fake.answers[f"repos/{REPO}/actions/workflows/"]["workflow_runs"][0]["event"] = event
    return Asking(fake)


class Asking:
    """A `Fake` that also records every path asked, to see what was consulted."""

    def __init__(self, fake: Fake):
        self.fake, self.seen = fake, []

    def __call__(self, path: str):
        self.seen.append(path)
        return self.fake(path)


def verdict(fake, workflow: str = "ci.yml"):
    return proved_tree.verdict(fake, REPO, workflow, SHA, "", "pull_request")


class Skips(unittest.TestCase):
    def test_a_merge_tree_a_dispatch_run_built(self):
        url, why = verdict(pr_fake())
        self.assertEqual(url, "http://run/7")
        self.assertIn("http://run/7", why)

    def test_a_merge_tree_a_push_run_built(self):
        self.assertEqual(verdict(pr_fake("push"))[0], "http://run/7")

    def test_lint_by_its_own_built_step(self):
        fake = pr_fake(jobs=[job("lint", "Clippy (hard gate, default features)")])
        self.assertEqual(verdict(fake, "lint.yml")[0], "http://run/7")

    def test_the_key_check_compares_against_the_merge_base(self):
        fake = pr_fake()
        verdict(fake)
        self.assertIn(f"repos/{REPO}/compare/{BASE}...{SHA}", fake.seen)


class RunsInFull(unittest.TestCase):
    def assert_runs(self, fake):
        url, why = verdict(fake)
        self.assertIsNone(url)
        self.assertTrue(why.startswith("runs in full"), why)

    def test_a_behind_head_whose_merge_tree_no_run_built(self):
        self.assert_runs(pr_fake(tree="x" * 40))

    def test_a_pull_request_run_of_the_same_tree_is_no_proof(self):
        self.assert_runs(pr_fake("pull_request"))

    def test_a_run_with_no_event_is_no_proof(self):
        self.assert_runs(pr_fake(None))

    def test_a_draft_or_no_op_run_is_no_proof(self):
        skipped = {b: job(b, None, "skipped") for b in ("build-test-cross", "blender", "deny")}
        skipped["build-test"] = job("build-test", "Test (engine, default)", step_concl="skipped")
        self.assert_runs(pr_fake(jobs=built_jobs(**skipped)))
        self.assert_runs(pr_fake(jobs=[job("changes", None), job("ci-gate", None)]))

    def test_a_cache_key_workflow_or_decider_change_against_the_base(self):
        for path in ("Cargo.lock", ".github/workflows/ci.yml", ".github/actions/install-tool/action.yml",
                     proved_tree.DECIDER):
            with self.subTest(path=path):
                self.assert_runs(pr_fake(files=("src/lib.rs", path)))

    def test_a_sha_that_is_not_a_merge(self):
        for parents in ((BASE,), (), (BASE, HEAD, "c" * 40)):
            with self.subTest(parents=parents):
                self.assert_runs(pr_fake(parents=parents))

    def test_any_lookup_error_or_malformed_answer(self):
        for prefix in (f"repos/{REPO}/compare/", f"repos/{REPO}/commits/{SHA}",
                       f"repos/{REPO}/actions/workflows/", f"repos/{REPO}/actions/runs/"):
            with self.subTest(prefix=prefix):
                self.assert_runs(pr_fake(fail=(prefix,)))
        fake = pr_fake()
        fake.fake.answers[f"repos/{REPO}/commits/{SHA}"] = {"commit": {"tree": {"sha": TREE}}}
        self.assert_runs(fake)


class NeverSuperseded(unittest.TestCase):
    """A merge commit is never `main`'s tip, so the push rule would skip every PR build."""

    def test_a_main_tip_other_than_the_merge_does_not_skip(self):
        fake = pr_fake(tree="x" * 40, tip="n" * 40)
        self.assertIsNone(verdict(fake)[0])
        self.assertNotIn(f"repos/{REPO}/commits/main", fake.seen)


if __name__ == "__main__":
    unittest.main()
