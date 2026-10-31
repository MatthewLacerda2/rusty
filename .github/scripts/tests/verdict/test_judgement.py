#!/usr/bin/env python3
"""The merge check refuses a pull request nothing actually checked.

The ruleset's required checks (#491) accept a draft's green gate on the same
commit, so this is the defence that matters: a check that only asked "did anything go red" would agree with GitHub about a draft's
run — gate green, nothing compiled. `judge` is tested by import because it is
pure and the interesting states are ones GitHub produces on its own schedule.
"""

from __future__ import annotations

import unittest

from merging.verdict import changes, decide
from tests.verdict.fixtures import CODE, READY, both, built, drafted, gates_running, job, run, verdict


class Judgement(unittest.TestCase):
    def assert_refused(self, said: tuple[bool, list[str]], because: str) -> None:
        ok, lines = said
        self.assertFalse(ok, lines)
        self.assertIn(because.lower(), " ".join(lines).lower())

    def test_a_genuine_pass_of_both_workflows_is_allowed(self):
        ok, lines = verdict(*both())
        self.assertTrue(ok, lines)
        self.assertIn("`ci` passed", lines[0])
        self.assertTrue(any("`lint` passed" in line for line in lines))

    def test_a_draft_is_refused(self):
        self.assert_refused(verdict(*both(), pull={**READY, "isDraft": True}), "draft")

    def test_no_run_at_all_is_refused(self):
        # The headline: not a red check but an absent one. `main`'s required
        # checks (#491) block it on GitHub too; the verdict refuses it first.
        self.assert_refused(verdict(), "no `ci` run exists")

    def test_green_ci_without_a_lint_run_is_half_a_check(self):
        self.assert_refused(verdict(run("ci")), "no `lint` run exists")

    def test_a_drafts_run_with_a_green_gate_is_refused(self):
        """scorsese#153 in rusty's shape: `ci-gate` runs `if: always()` and
        passes over skipped jobs, so the gate alone would call this a pass."""
        ci, lint = both()
        self.assert_refused(
            verdict(ci, lint, jobs={1: drafted("ci"), 2: built("lint")}), "ran a gated job"
        )

    def test_changes_succeeding_is_not_a_gated_job_running(self):
        ci, lint = both()
        jobs = {1: [job("changes", "success"), job("ci-gate", "success")], 2: built("lint")}
        self.assert_refused(verdict(ci, lint, jobs=jobs), "ran a gated job")

    def test_a_gated_job_without_its_gate_is_not_a_pass(self):
        ci, lint = both()
        jobs = {1: [job("build-test", "success")], 2: built("lint")}
        # A gate not yet listed is a run still going (#1095): refused, as wait.
        self.assert_refused(verdict(ci, lint, jobs=jobs), "no `ci-gate` job is listed yet")

    def test_a_failed_run_is_refused_and_points_at_it(self):
        ok, lines = verdict(*both(conclusion="failure"))
        self.assertFalse(ok)
        self.assertIn("failure", " ".join(lines))
        self.assertIn("https://example.invalid/run/1", " ".join(lines))

    def test_a_run_in_flight_is_refused(self):
        ci, lint = both(status="in_progress", conclusion=None)
        self.assert_refused(verdict(ci, lint, jobs={1: gates_running(), 2: built("lint")}), "in_progress")

    def test_a_failure_outranks_an_absence(self):
        # A red `ci` with no `lint` yet is answered: it is red.
        self.assert_refused(verdict(run("ci", conclusion="failure")), "concluded failure")

    def test_a_green_run_beside_a_red_one_is_not_a_pass(self):
        # scorsese#245, in the direction that matters, and in both orders.
        green, red = run("ci", id=1), run("ci", id=3, conclusion="failure")
        lint = run("lint")
        jobs = {1: built("ci"), 3: built("ci"), 2: built("lint")}
        first, _ = decide.judge(READY, [green, red, lint], jobs, CODE)
        second, _ = decide.judge(READY, [red, green, lint], jobs, CODE)
        self.assertFalse(first)
        self.assertEqual(first, second)

    def test_an_older_failure_is_never_forgiven_by_a_newer_pass(self):
        red = run("ci", id=1, conclusion="failure", created_at="2026-09-30T00:00:00Z")
        green = run("ci", id=3)
        jobs = {1: built("ci"), 3: built("ci"), 2: built("lint")}
        self.assertFalse(verdict(red, green, run("lint"), jobs=jobs)[0])

    def test_a_green_run_beside_a_skipped_draft_run_is_a_pass(self):
        # scorsese#245, in the direction that cost a wasted commit.
        draft = run("ci", id=1, created_at="2026-09-30T00:00:00Z")
        ready = run("ci", id=3)
        jobs = {1: drafted("ci"), 3: built("ci"), 2: built("lint")}
        ok, lines = verdict(draft, ready, run("lint"), jobs=jobs)
        self.assertTrue(ok, lines)
        self.assertIn("2 `ci` runs exist", " ".join(lines))


class NoRun(unittest.TestCase):
    def said(self, **pull: object) -> str:
        return " ".join(decide.judge({**READY, **pull}, [], {}, CODE)[1])

    def test_a_conflicted_branch_is_told_it_conflicts_not_told_about_153(self):
        said = self.said(mergeable="CONFLICTING", mergeStateStatus="DIRTY")
        self.assertIn("conflicts with `main`", said)
        self.assertNotIn("empty commit", said)

    def test_either_field_alone_reads_a_conflict(self):
        for field, value in (("mergeable", "CONFLICTING"), ("mergeStateStatus", "DIRTY")):
            with self.subTest(field=field):
                self.assertIn("conflicts with `main`", self.said(**{field: value}))

    def test_a_clean_branch_is_warned_off_the_workflow_file(self):
        said = self.said()
        self.assertIn("startup_failure", said)
        self.assertIn("scorsese#153", said)

    def test_unknown_mergeability_is_not_read_as_no_conflict(self):
        self.assertIn("has not finished computing", self.said(mergeable="UNKNOWN"))

    def test_a_branch_merely_behind_is_named_not_blamed(self):
        self.assertIn("BEHIND", self.said(mergeStateStatus="BEHIND"))


class Markdown(unittest.TestCase):
    def test_a_markdown_only_pass_says_nothing_compiled_by_design(self):
        ok, lines = verdict(*both(), files=["README.md", "docs/testing.md"])
        self.assertTrue(ok, lines)
        self.assertIn("Markdown-only", " ".join(lines))

    def test_the_api_doc_is_not_markdown_only_for_merge_purposes(self):
        """#525: CI's `code` filter skips the drift tests on this exact PR."""
        ci, lint = both()
        skipped = [job("build-test", "success", [{"name": "Test (engine, dev features)", "conclusion": "skipped"}]), job("ci-gate", "success")]
        ok, lines = verdict(ci, lint, jobs={1: skipped, 2: built("lint")}, files=["docs/api/Physics.md"])
        self.assertFalse(ok, lines)
        self.assertIn("#525", " ".join(lines))

    def test_the_api_doc_passes_once_the_drift_tests_really_ran(self):
        ok, lines = verdict(*both(), files=["docs/api/Physics.md", "README.md"])
        self.assertTrue(ok, lines)

    def test_the_api_doc_beside_code_is_just_code(self):
        self.assertFalse(changes.needs_drift_run(["docs/api/Physics.md", "src/api/mod.rs"]))


if __name__ == "__main__":
    unittest.main()
