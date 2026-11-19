"""Every job runs on a pinned image, and every Ubuntu one on the gate's.

It reaches past the gating workflows (`test_workflow_hygiene.py` keeps their
other rules) since #2385: a shadowed spot's
fog shaft went missing twice in mutation Baselines on `ubuntu-latest` (Mesa 26's
lavapipe) and never in `ci.yml`, on `ubuntu-24.04` (Mesa 25), so a signal that
audits the tests ran them on a driver the gate never proves. Every workflow's
jobs run on a pinned image, and every Ubuntu one on the image `ci.yml`'s
`build-test` names; a `${{ matrix.<key> }}` is checked through the key's values
written in the job (`tests.workflows.runs_on`), and a `runs-on` the reader cannot
read (a list, a mapping, another expression) fails naming its line, never
passes. A cache key that names an Ubuntu image names that one too: rust-cache's
own key names the OS and the architecture, not the image, so a `target/` built
on a newer glibc would restore on the gate's.
"""

from __future__ import annotations

import re
import unittest

from tests.workflows import COMMENT, Unreadable, parse, repo_workflows, runs_on

#: The job whose image is the one definition; its comment says what a move costs.
GATE = ("ci.yml", "build-test")
UBUNTU = re.compile(r"\bubuntu-(?:\d+\.\d+|latest)\b")


def gate_image(workflows: dict[str, str]) -> str:
    workflow, name = GATE
    job = parse(workflows[workflow]).get(name)
    images = runs_on(job) if job else None
    assert images and len(images) == 1, f"{workflow}: job `{name}` must name one `runs-on` image"
    return images[0]


def image_faults(workflows: dict[str, str]) -> list[str]:
    """One message per job that runs on a moving image (`-latest`), on an Ubuntu
    image other than the gate's, or on a `runs-on` the reader cannot read."""
    gate, faults = gate_image(workflows), []
    for workflow, text in sorted(workflows.items()):
        for name, job in parse(text).items():
            try:
                images = runs_on(job) or []
            except Unreadable as e:
                faults.append(f"{workflow}:{e.line}: job `{name}`: {e.why}; "
                              "write `runs-on: <image>` (or `${{ matrix.<key> }}` with the key's values in the job)")
                continue
            for number, line in job.lines:  # a cache key names the image its `target/` was built on
                for named in set(UBUNTU.findall(COMMENT.sub("", line))) - {gate}:
                    if not line.startswith("    runs-on:"):
                        faults.append(f"{workflow}:{number}: job `{name}` names `{named}`, not the gate's "
                                      f"`{gate}`: a key naming another image restores what it built; write `{gate}`.")
            for image in images:
                if image.endswith("-latest") or (image.startswith("ubuntu") and image != gate):
                    faults.append(
                        f"{workflow}: job `{name}` runs on `{image}`, not the gate's `{gate}`. Every Ubuntu "
                        f"job runs on the one image `{GATE[0]}`'s `{GATE[1]}` names, so the tests run on the "
                        f"lavapipe the gate proves, and none on a `-latest` that moves under an unchanged "
                        f"diff; write `runs-on: {gate}`, or move the image there, for every job at once.")
    return faults


def workflow(*jobs: tuple[str, str]) -> str:
    return "on: push\njobs:\n" + "".join(f"  {n}:\n    runs-on: {i}\n    steps: []\n" for n, i in jobs)


def flows(*jobs: tuple[str, str]) -> dict[str, str]:
    """The gate's workflow and one more, `other.yml`, holding `jobs`."""
    return {"ci.yml": workflow(("build-test", "ubuntu-24.04")), "other.yml": workflow(*jobs)}


class OnePinnedImage(unittest.TestCase):
    def test_every_job_runs_on_the_gates_pinned_image(self):
        self.assertEqual(image_faults(repo_workflows()), [])
        self.assertRegex(gate_image(repo_workflows()), r"^ubuntu-\d+\.\d+$")

    def test_a_moving_or_other_ubuntu_image_is_flagged_by_job_and_image(self):
        for image in ("ubuntu-latest", "'ubuntu-22.04'", '"ubuntu-latest" # newest', "macos-latest"):
            with self.subTest(image=image):
                [fault] = image_faults(flows(("plan", "ubuntu-24.04"), ("m", image)))
                self.assertIn(f"other.yml: job `m` runs on `{image.split()[0].strip(chr(39) + chr(34))}`", fault)

    def test_a_runs_on_it_cannot_read_fails_naming_file_and_line(self):
        for image in ("[ubuntu-latest]", "${{ inputs.os }}", "${{ matrix.os }}", ""):
            with self.subTest(image=image):
                [fault] = image_faults(flows(("m", image)))
                self.assertTrue(fault.startswith("other.yml:4: job `m`"), fault)

    def test_a_cache_key_naming_another_image_is_flagged(self):
        for key, faults in (("mutants-ubuntu-24.04", 0), ("mutants-ubuntu-26.04", 1), ("mutants # ubuntu-latest", 0)):
            with self.subTest(key=key):
                cached = workflow(("m", "ubuntu-24.04")).replace("    steps: []\n", f"    with:\n      shared-key: {key}\n")
                found = image_faults({**flows(), "other.yml": cached})
                self.assertEqual(len(found), faults, found)
                self.assertTrue(all(f.startswith("other.yml:6: job `m` names `ubuntu-26.04`") for f in found))

    def test_a_commented_heading_does_not_hide_the_next_job(self):
        text = "on: push\njobs:\n  a: # first\n    runs-on: ubuntu-24.04\n  b: # second\n    runs-on: ubuntu-latest\n"
        self.assertEqual(list(parse(text)), ["a", "b"])
        [fault] = image_faults({"ci.yml": workflow(("build-test", "ubuntu-24.04 # the gate")), "w.yml": text})
        self.assertIn("job `b` runs on `ubuntu-latest`", fault)

    def test_a_matrix_of_pinned_images_and_the_gate_pass(self):
        matrix = "    strategy:\n      matrix:\n        include:\n          - { os: macos-26, shard: 1/2 }\n" \
                 "          - os: ubuntu-24.04\n"
        cross = workflow(("cross", "${{ matrix.os }}")).replace("    steps: []\n", matrix)
        self.assertEqual(image_faults({**flows(("job", "ubuntu-24.04"), ("mac", "macos-26")), "x.yaml": cross}), [])
        self.assertEqual(runs_on(parse(cross)["cross"]), ["macos-26", "ubuntu-24.04"])



if __name__ == "__main__":
    unittest.main()
