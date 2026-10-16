#!/usr/bin/env python3
"""The full mutation sweep runs on `main` and nowhere else (#972).

A sweep holds five runners for most of a day (#705), so `mutants-plan` refuses
any other ref before it looks at anything. These tests run that step's own
shell, lifted from the workflow, with `gh` stubbed to fail: on `main` a failed
lookup sweeps (fail open), so the refusal is the only way to get no sweep.
"""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[2] / "workflows" / "mutants-sweep.yml"


def plan_script() -> str:
    """The `run: |` body of the step with `id: plan`, dedented."""
    after = WORKFLOW.read_text().split("        id: plan\n", 1)[1]
    body = after.split("        run: |\n", 1)[1]
    lines = []
    for line in body.splitlines():
        if line.strip() and not line.startswith("          "):
            break
        lines.append(line[10:])
    return "\n".join(lines) + "\n"


class Plan(unittest.TestCase):
    def run_plan(self, ref: str, force: str = "false") -> tuple[int, str]:
        with tempfile.TemporaryDirectory() as tmp:
            stub = Path(tmp) / "gh"
            stub.write_text("#!/bin/sh\nexit 1\n")
            stub.chmod(0o755)
            out, summary = Path(tmp) / "out", Path(tmp) / "summary"
            env = dict(os.environ, PATH=f"{tmp}:{os.environ['PATH']}", REF=ref, FORCE=force,
                       REPO="o/r", SHA="abc", GITHUB_OUTPUT=str(out), GITHUB_STEP_SUMMARY=str(summary))
            done = subprocess.run(["bash", "-e", "-c", plan_script()], env=env,
                                  capture_output=True, text=True, check=False)
            return done.returncode, out.read_text() if out.exists() else ""

    def test_a_branch_is_refused_red(self):
        rc, out = self.run_plan("refs/heads/972-feature")
        self.assertNotEqual(rc, 0)
        self.assertNotIn("sweep=true", out)

    def test_force_does_not_open_a_branch(self):
        rc, out = self.run_plan("refs/heads/972-feature", force="true")
        self.assertNotEqual(rc, 0)
        self.assertNotIn("sweep=true", out)

    def test_main_sweeps(self):
        # The schedule and a dispatch on main both carry this ref.
        self.assertEqual(self.run_plan("refs/heads/main"), (0, "sweep=true\n"))
        self.assertEqual(self.run_plan("refs/heads/main", force="true"), (0, "sweep=true\n"))

    def test_the_guard_reads_the_ref_the_run_was_started_on(self):
        self.assertRegex(WORKFLOW.read_text(), re.escape("REF: ${{ github.ref }}"))


if __name__ == "__main__":
    unittest.main()
