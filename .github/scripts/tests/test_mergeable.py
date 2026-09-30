#!/usr/bin/env python3
"""The merge check refuses a pull request nothing actually checked.

`main` has no required checks (#491), so this is the whole defence: a check
that only asked "did anything go red" would agree with GitHub about a draft's
run — gate green, nothing compiled. `judge` is tested by import because it is
pure and the interesting states are ones GitHub produces on its own schedule;
the workflow files are read directly so the gate table cannot drift from them.
"""

from __future__ import annotations

import importlib.util
import re
import subprocess
import sys
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
SCRIPT = SCRIPTS / "mergeable.py"
WORKFLOW_DIR = SCRIPTS.parent / "workflows"

_spec = importlib.util.spec_from_file_location("mergeable", SCRIPT)
mergeable = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(mergeable)

SHA = "c23b7c4f0000000000000000000000000000000a"
READY = {
    "isDraft": False,
    "headRefOid": SHA,
    "number": 524,
    "mergeable": "MERGEABLE",
    "mergeStateStatus": "CLEAN",
}
CODE = ["src/app/mod.rs"]


def run(workflow: str = "ci", **fields: object) -> dict:
    """A completed, successful run of `workflow` on [`SHA`], overridden by field."""
    return {
        "name": workflow,
        "head_sha": SHA,
        "id": 1 if workflow == "ci" else 2,
        "created_at": "2026-09-30T01:00:00Z",
        "status": "completed",
        "conclusion": "success",
        "html_url": "https://example.invalid/run/1",
    } | fields


def job(name: str, conclusion: str, steps: list[dict] | None = None) -> dict:
    return {"name": name, "conclusion": conclusion, "steps": steps or []}


def built(workflow: str) -> list[dict]:
    """The jobs of a run that genuinely checked a code change."""
    if workflow == "ci":
        return [
            job("changes", "success"),
            job("build-test", "success", [{"name": "Test (engine, dev features)", "conclusion": "success"}]),
            job("build-test-cross (macos-latest)", "success"),
            job("deny", "success"),
            job("ci-gate", "success"),
            job("coverage", "skipped"),
        ]
    return [job("changes", "success"), job("lint", "success"), job("lint-gate", "success")]


def drafted(workflow: str) -> list[dict]:
    """A draft's run: `changes` and the gate green, every gated job skipped."""
    gate, gated = mergeable.WORKFLOWS[workflow]
    return [job("changes", "success"), *(job(g, "skipped") for g in gated), job(gate, "success")]


def verdict(*runs: dict, jobs: dict | None = None, files=CODE, pull=None):
    """[`mergeable.judge`], each run's jobs defaulting to a genuine build."""
    jobs = jobs if jobs is not None else {r["id"]: built(r["name"]) for r in runs}
    return mergeable.judge(pull or READY, list(runs), jobs, files)


def both(**ci: object) -> tuple[dict, dict]:
    return run("ci", **ci), run("lint")


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
        # The headline: not a red check but an absent one — and with no
        # required checks on `main`, GitHub would merge it.
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
        self.assert_refused(verdict(ci, lint, jobs=jobs), "ran a gated job")

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
        first, _ = mergeable.judge(READY, [green, red, lint], jobs, CODE)
        second, _ = mergeable.judge(READY, [red, green, lint], jobs, CODE)
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


def gates_running() -> list[dict]:
    """A `ci` run whose gated jobs, and so its gate, are still going."""
    return [job("changes", "success"), job("build-test", None), job("deny", "success"), job("ci-gate", None)]


def signalling(conclusion: str | None) -> list[dict]:
    """A `ci` run whose gates concluded green beside `mutants-pr` in `conclusion`."""
    return [*built("ci"), job("mutants-pr", conclusion), job("coverage-pr", "success")]


class Signals(unittest.TestCase):
    """#555: `mutants-pr` shares the `ci` run but is no gate's need."""

    def in_flight(self, jobs: list[dict], **ci: object):
        ci, lint = both(status="in_progress", conclusion=None, **ci)
        return verdict(ci, lint, jobs={1: jobs, 2: built("lint")})

    def test_green_gates_beside_a_running_mutation_job_are_mergeable(self):
        ok, lines = self.in_flight(signalling(None))
        self.assertTrue(ok, lines)
        self.assertIn("Still running, and not gating: mutants-pr", " ".join(lines))

    def test_gates_still_running_are_refused_whatever_the_signals_do(self):
        ok, lines = self.in_flight([*gates_running(), job("mutants-pr", "success")])
        self.assertFalse(ok, lines)
        self.assertIn("in_progress", " ".join(lines))

    def test_a_red_signal_never_blocks(self):
        # Mid-run, and after: `continue-on-error` leaves the finished run green.
        self.assertTrue(self.in_flight(signalling("failure"))[0])
        ok, lines = verdict(*both(), jobs={1: signalling("failure"), 2: built("lint")})
        self.assertTrue(ok, lines)

    def test_a_finished_red_run_is_not_forgiven_by_a_green_gate(self):
        ok, _ = verdict(*both(conclusion="failure"), jobs={1: signalling("success"), 2: built("lint")})
        self.assertFalse(ok)

    def test_a_red_gate_beside_a_running_signal_is_a_failure_now(self):
        jobs = [*built("ci")[:-2], job("ci-gate", "failure"), job("mutants-pr", None)]
        ok, lines = self.in_flight(jobs)
        self.assertFalse(ok, lines)
        self.assertIn("concluded failure", " ".join(lines))

    def test_a_drafts_run_is_still_unbuilt_while_a_signal_runs(self):
        ok, lines = self.in_flight([*drafted("ci"), job("mutants-pr", None)])
        self.assertFalse(ok, lines)
        self.assertIn("ran a gated job", " ".join(lines))

    def test_the_api_doc_still_needs_its_drift_run_while_a_signal_runs(self):
        skipped = [
            job("build-test", "success", [{"name": "Test (engine, dev features)", "conclusion": "skipped"}]),
            job("ci-gate", "success"),
            job("mutants-pr", None),
        ]
        ok, lines = self.in_flight(skipped)
        self.assertTrue(ok, lines)  # code files: the drift rule does not apply
        ci, lint = both(status="in_progress", conclusion=None)
        ok, lines = verdict(ci, lint, jobs={1: skipped, 2: built("lint")}, files=["docs/api/Physics.md"])
        self.assertFalse(ok, lines)
        self.assertIn("#525", " ".join(lines))

    def test_a_cancelled_run_with_a_skipped_gate_still_reads_cancelled(self):
        ci, lint = both(conclusion="cancelled")
        jobs = {1: [job("build-test", "cancelled"), job("ci-gate", "skipped")], 2: built("lint")}
        ok, lines = verdict(ci, lint, jobs=jobs)
        self.assertFalse(ok)
        self.assertIn("concluded cancelled", " ".join(lines))


class Cancelled(unittest.TestCase):
    """#482 cancels a superseded PR run; until #515 its gate reads red."""

    def test_a_cancelled_run_superseded_on_the_same_commit_is_dropped(self):
        old = run("ci", id=1, conclusion="cancelled", created_at="2026-09-30T00:00:00Z")
        new = run("ci", id=3)
        jobs = {1: [job("ci-gate", "failure")], 3: built("ci"), 2: built("lint")}
        ok, lines = verdict(old, new, run("lint"), jobs=jobs)
        self.assertTrue(ok, lines)
        self.assertIn("1 cancelled and superseded", " ".join(lines))

    def test_the_order_listed_does_not_decide_which_is_newer(self):
        old = run("ci", id=1, conclusion="cancelled", created_at="2026-09-30T00:00:00Z")
        new = run("ci", id=3)
        jobs = {1: [], 3: built("ci"), 2: built("lint")}
        self.assertTrue(verdict(new, old, run("lint"), jobs=jobs)[0])
        self.assertTrue(verdict(old, new, run("lint"), jobs=jobs)[0])

    def test_a_cancelled_run_that_is_the_newest_still_refuses(self):
        # Nothing replaced its answer, so nothing has answered.
        ok, lines = verdict(*both(conclusion="cancelled"))
        self.assertFalse(ok)
        self.assertIn("cancelled", " ".join(lines))

    def test_superseded_by_a_live_run_means_wait_for_that_run(self):
        old = run("ci", id=1, conclusion="cancelled", created_at="2026-09-30T00:00:00Z")
        new = run("ci", id=3, status="in_progress", conclusion=None)
        ok, lines = verdict(old, new, run("lint"), jobs={1: [], 3: [], 2: built("lint")})
        self.assertFalse(ok)
        self.assertIn("in_progress", " ".join(lines))

    def test_558s_run_set_waits_for_the_live_run_not_red(self):
        # #562, verbatim from #558's head: same workflow, same second, and the
        # cancelled run carries the *higher* id — id order cannot decide it.
        at = "2026-09-30T15:44:14Z"
        live = run("lint", id=36738997580, created_at=at, status="in_progress", conclusion=None)
        dead = run("lint", id=36738997790, created_at=at, conclusion="cancelled")
        ci_dead = run("ci", id=36738997830, created_at=at, conclusion="cancelled")
        ci_live = run("ci", id=36738998031, created_at="2026-09-30T15:44:15Z", status="pending", conclusion=None)
        jobs = {r["id"]: [] for r in (live, dead, ci_dead, ci_live)}
        ok, lines = verdict(live, dead, ci_dead, ci_live, jobs=jobs)
        self.assertFalse(ok)
        self.assertIn("wait for it", " ".join(lines).lower())
        self.assertNotIn("beside a red one", " ".join(lines))

    def test_558s_run_set_passes_once_the_live_run_is_green(self):
        at = "2026-09-30T15:44:14Z"
        done = run("lint", id=36738997580, created_at=at)
        dead = run("lint", id=36738997790, created_at=at, conclusion="cancelled")
        jobs = {1: built("ci"), done["id"]: built("lint"), dead["id"]: []}
        ok, lines = verdict(run("ci"), done, dead, jobs=jobs)
        self.assertTrue(ok, lines)

    def test_a_cancelled_run_newer_than_a_green_one_still_refuses(self):
        green = run("ci", id=1, created_at="2026-09-30T00:00:00Z")
        dead = run("ci", id=3, conclusion="cancelled")
        ok, lines = verdict(green, dead, run("lint"), jobs={1: built("ci"), 3: [], 2: built("lint")})
        self.assertFalse(ok)
        self.assertIn("cancelled", " ".join(lines))

    def test_cancelled_runs_never_supersede_each_other(self):
        at = "2026-09-30T00:00:00Z"
        a, b = (run("ci", id=i, created_at=at, conclusion="cancelled") for i in (1, 3))
        self.assertEqual(mergeable.superseded([a, b]), [])

    def test_only_cancelled_runs_are_ever_superseded(self):
        red = run("ci", id=1, conclusion="failure", created_at="2026-09-30T00:00:00Z")
        self.assertEqual(mergeable.superseded([red, run("ci", id=3)]), [])


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
        self.assertFalse(mergeable.needs_drift_run(["docs/api/Physics.md", "src/api/mod.rs"]))


class NoRun(unittest.TestCase):
    def said(self, **pull: object) -> str:
        return " ".join(mergeable.judge({**READY, **pull}, [], {}, CODE)[1])

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


class Picking(unittest.TestCase):
    def test_a_run_for_another_commit_is_not_this_commits(self):
        self.assertEqual(mergeable.runs_for([run(head_sha="0" * 40)], SHA, "ci"), [])

    def test_other_workflows_are_not_asked_about(self):
        # `main-health` is a `workflow_run` on `main`, never a PR check.
        for name in ("main-health", "docs"):
            self.assertEqual(mergeable.runs_for([run(name)], SHA, "ci"), [])

    def test_a_matrix_job_counts_under_its_base_name(self):
        self.assertEqual(mergeable.base("build-test-cross (windows-latest)"), "build-test-cross")

    def test_a_skipped_run_is_not_a_failed_one(self):
        self.assertEqual(mergeable.failed_runs([run(conclusion="skipped")]), [])


def gate_needs(workflow_file: str, gate: str) -> list[str]:
    """The `needs:` of `gate` in a workflow file — flow-style list, one line."""
    text = (WORKFLOW_DIR / workflow_file).read_text()
    found = re.search(rf"^  {re.escape(gate)}:\n    needs: \[([^\]]*)\]", text, re.M)
    assert found, f"{workflow_file} has no `{gate}` job with a flow-style `needs:`"
    return [n.strip() for n in found.group(1).split(",")]


class Workflows(unittest.TestCase):
    """The table in `mergeable.py` is held to the workflow files it describes."""

    FILES = {"ci": "ci.yml", "lint": "lint.yml"}

    def test_each_gates_needs_are_changes_plus_the_gated_jobs(self):
        # A gating job added to a gate's `needs:` and not here would be one a
        # pass never asks about. `changes` is excluded on purpose (see WORKFLOWS).
        for workflow, (gate, gated) in mergeable.WORKFLOWS.items():
            with self.subTest(workflow=workflow):
                needs = gate_needs(self.FILES[workflow], gate)
                self.assertEqual(sorted(needs), sorted(("changes", *gated)))

    def test_each_workflows_name_is_the_one_the_runs_api_reports(self):
        for workflow, file in self.FILES.items():
            with self.subTest(workflow=workflow):
                first = (WORKFLOW_DIR / file).read_text().splitlines()[0]
                self.assertEqual(first, f"name: {workflow}")

    def test_build_test_still_has_the_step_the_drift_check_looks_for(self):
        text = (WORKFLOW_DIR / "ci.yml").read_text()
        body = text.split(f"\n  {mergeable.DRIFT_JOB}:\n", 1)[1].split("\n  build-test-cross:\n", 1)[0]
        names = [n.lower() for n in re.findall(r"- name: (.+)", body)]
        self.assertTrue(any(all(w in n for w in mergeable.DRIFT_STEP_WORDS) for n in names), names)


class Contract(unittest.TestCase):
    def test_it_asks_for_a_pull_request_number(self):
        done = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True, check=False)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("usage", done.stderr.lower())


if __name__ == "__main__":
    unittest.main()
