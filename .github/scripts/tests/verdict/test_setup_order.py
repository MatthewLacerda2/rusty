"""#1932: [`setup.SETUP`] is held to the workflows' gated jobs, in order.

`setup.failed_in_setup` trusts a failed setup step only when every step before
it is a setup step too, so a step inserted before an install (PR #1933's
review: `Driver version (Mesa / lavapipe)` before `Install cargo-nextest`)
silently hands that install's 504 back to the code. Read each gated job's steps
in order, by the name the jobs API reports them under: `name:` when given, else
`Run <uses>`. Only a step's own `name:` / `uses:` keys count, never a job's name
or a `with:` key.
"""

from __future__ import annotations

import pathlib
import re
import unittest

from merging.verdict import runlist, setup

WORKFLOWS = pathlib.Path(__file__).resolve().parents[3] / "workflows"
# GitHub's own first step, and a Docker action's image build before checkout:
# neither is written as a step in a workflow.
SYNTHETIC = ("Set up job", "Build EmbarkStudios/cargo-deny-action@v2")


def jobs(text: str) -> dict[str, list[str]]:
    """Each job's steps, as the jobs API names them, in order (2-space YAML)."""
    found: dict[str, list[str]] = {}
    job, steps, step = None, None, None
    for line in text.splitlines():
        if m := re.match(r"^  ([\w-]+):\s*$", line):
            job, steps, step = m.group(1), None, None
        elif job and re.match(r"^    steps:\s*$", line):
            steps = found.setdefault(job, [])
        elif steps is not None and (m := re.match(r"^      (- |  )(name|uses):\s*(.+?)\s*$", line)):
            if m.group(1) == "- ":
                step = {}
                steps.append(step)
            step[m.group(2)] = m.group(3)
        elif steps is not None and line.startswith("      - "):
            step = {}
            steps.append(step)
    return {j: [s.get("name") or f"Run {s.get('uses')}" for s in listed] for j, listed in found.items()}


def gated() -> dict[str, list[str]]:
    """Every gated job of [`runlist.WORKFLOWS`], keyed `workflow/job`."""
    out = {}
    for workflow, (_gate, names) in runlist.WORKFLOWS.items():
        steps = jobs((WORKFLOWS / f"{workflow}.yml").read_text())
        out |= {f"{workflow}/{name}": steps[name] for name in names}
    return out


def unlisted_before_setup(steps: list[str]) -> list[str]:
    """The steps before the job's last setup step that are not setup steps."""
    last = max((i for i, s in enumerate(steps) if s in setup.SETUP), default=-1)
    return [s for s in steps[:last] if s not in setup.SETUP]


class Order(unittest.TestCase):
    def test_no_step_before_a_setup_step_is_unlisted(self):
        for job, steps in gated().items():
            with self.subTest(job=job):
                self.assertEqual(unlisted_before_setup(steps), [], f"{job}: list these in setup.SETUP or move them after")

    def test_the_review_case_is_flagged(self):
        steps = ["Run actions/checkout@v7", "Toolchain (from rust-toolchain.toml)", "Driver version (Mesa / lavapipe)",
                 "Install cargo-nextest", "Test (engine, default)"]
        self.assertEqual(unlisted_before_setup(steps), [])
        printed = [*steps[:3], "Print the version", *steps[3:]]
        self.assertEqual(unlisted_before_setup(printed), ["Print the version"])

    def test_every_name_is_still_a_gated_jobs_step(self):
        reported = {s for steps in gated().values() for s in steps}
        for name in setup.SETUP:
            if name not in SYNTHETIC:
                with self.subTest(name=name):
                    self.assertIn(name, reported, f"no gated job's step reports as {name!r}")

    def test_only_steps_keys_are_read(self):
        text = "jobs:\n  a:\n    name: Install cargo-nextest\n    steps:\n      - uses: x/y@v1\n        with:\n" \
               "          name: Toolchain (from rust-toolchain.toml)\n      - run: echo\n        name: Test\n"
        self.assertEqual(jobs(text), {"a": ["Run x/y@v1", "Test"]})


if __name__ == "__main__":
    unittest.main()
