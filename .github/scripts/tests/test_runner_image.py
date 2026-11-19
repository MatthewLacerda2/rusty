#!/usr/bin/env python3
"""Every Ubuntu job runs on the one image the gate proves (#2385).

A shadowed spot's fog shaft went missing twice in mutation Baselines on
`ubuntu-latest` (Mesa 26's lavapipe) and never in `ci.yml`, on `ubuntu-24.04`
(Mesa 25): a signal that audits the tests (mutation, coverage, `main`'s
health) ran them on a software rasteriser the gate never proves, and paid a
false baseline failure for it. So the image is named once, by `ci.yml`'s
`build-test` job, and every other Ubuntu `runs-on` in `.github/workflows/`
names the same one. A `${{ matrix.* }}` value and a macOS leg pass: neither
is an Ubuntu image written in place.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

WORKFLOW_DIR = Path(__file__).resolve().parents[2] / "workflows"
#: The job whose image is the one definition; its comment says what a move costs.
GATE = ("ci.yml", "build-test")

JOB = re.compile(r"^  ([\w-]+):\n", re.M)
RUNS_ON = re.compile(r"^    runs-on: *(\S+) *$", re.M)


def images(text: str) -> dict[str, str]:
    """Each top-level job's `runs-on` value, from the workflow's text."""
    body = text.split("\njobs:\n", 1)[1]
    parts = JOB.split(body)
    found = {}
    for name, job in zip(parts[1::2], parts[2::2]):
        if m := RUNS_ON.search(job):
            found[name] = m[1]
    return found


def gate_image(workflows: dict[str, str]) -> str:
    workflow, job = GATE
    return images(workflows[workflow])[job]


def strays(workflows: dict[str, str]) -> list[str]:
    """One message per Ubuntu job that names another image than the gate's."""
    gate = gate_image(workflows)
    return [
        f"{workflow}: job `{job}` runs on `{image}`, not the gate's `{gate}`. "
        f"Every Ubuntu job runs on the one image `{GATE[0]}`'s `{GATE[1]}` names, "
        f"so the tests run on the lavapipe the gate proves; write `runs-on: {gate}`, "
        f"or move the image there, for every job at once."
        for workflow, text in sorted(workflows.items())
        for job, image in images(text).items()
        if image.startswith("ubuntu") and image != gate
    ]


def repo_workflows() -> dict[str, str]:
    return {p.name: p.read_text() for p in WORKFLOW_DIR.glob("*.yml")}


def workflow(*jobs: tuple[str, str]) -> str:
    return "on: push\njobs:\n" + "".join(f"  {name}:\n    runs-on: {image}\n    steps: []\n" for name, image in jobs)


class OneUbuntuImage(unittest.TestCase):
    def test_the_repository_names_one(self):
        self.assertEqual(strays(repo_workflows()), [])

    def test_the_gate_names_a_pinned_ubuntu_image(self):
        self.assertRegex(gate_image(repo_workflows()), r"^ubuntu-\d+\.\d+$")

    def test_another_ubuntu_image_is_flagged_by_job_and_image(self):
        flows = {
            "ci.yml": workflow(("build-test", "ubuntu-24.04")),
            "mutants.yml": workflow(("plan", "ubuntu-24.04"), ("mutants", "ubuntu-latest")),
        }
        [message] = strays(flows)
        self.assertIn("mutants.yml: job `mutants` runs on `ubuntu-latest`", message)
        self.assertIn("`ubuntu-24.04`", message)

    def test_a_matrix_value_and_a_macos_leg_pass(self):
        flows = {
            "ci.yml": workflow(("build-test", "ubuntu-24.04"), ("cross", "${{ matrix.os }}"), ("mac", "macos-26")),
            "other.yml": workflow(("job", "ubuntu-24.04")),
        }
        self.assertEqual(strays(flows), [])


if __name__ == "__main__":
    unittest.main()
