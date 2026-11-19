#!/usr/bin/env python3
"""A job that checks out an older commit runs the workflow's own scripts.

A queue request checks out its squash commit, which can predate a script or
subcommand a step calls (`refuse`): that commit's copy would exit 2 and the
report would blame planning. So every job that checks out `inputs.commit` and
runs a `.github/scripts/` script restores `.github/scripts` from
`$GITHUB_SHA` first.
"""

from __future__ import annotations

import re
import unittest

from tests import SCRIPTS

WORKFLOW = SCRIPTS.parent / "workflows" / "mutants-on-request.yml"


def jobs(text: str) -> dict[str, str]:
    """Each job's text under `jobs:`, by name."""
    body = text[text.index("\njobs:\n"):]
    cuts = list(re.finditer(r"^  ([\w-]+):\n", body, re.M))
    return {m[1]: body[m.end():n.start() if n else len(body)] for m, n in zip(cuts, [*cuts[1:], None])}


def restores_scripts_first(job: str) -> bool:
    restore = re.search(r'git checkout "\$GITHUB_SHA" -- [^\n]*\.github/scripts', job)
    return bool(restore) and restore.start() < job.index("python3 .github/scripts/")


class OlderCommit(unittest.TestCase):
    def test_every_job_on_inputs_commit_restores_the_scripts_before_running_one(self) -> None:
        found = jobs(WORKFLOW.read_text())
        checked = [name for name, job in found.items()
                   if "ref: ${{ inputs.commit }}" in job and "python3 .github/scripts/" in job]
        self.assertGreaterEqual(set(checked), {"plan", "mutants"})
        for name in checked:
            self.assertTrue(restores_scripts_first(found[name]),
                            f"job `{name}` checks out inputs.commit and runs a script from it: "
                            "add .github/scripts to its `git checkout \"$GITHUB_SHA\" --` step")

    def test_the_check_flags_a_job_that_runs_the_commit_s_copy(self) -> None:
        bad = ('      - run: git checkout "$GITHUB_SHA" -- .github/actions\n'
               "      - run: python3 .github/scripts/mutants-request.py refuse plan.json\n")
        self.assertFalse(restores_scripts_first(bad))
        self.assertTrue(restores_scripts_first(bad.replace("actions\n", "actions .github/scripts\n")))


if __name__ == "__main__":
    unittest.main()
