#!/usr/bin/env python3
"""The gating workflows keep the shape #930 gave them.

Each rule here protects the merge queue from a run it cannot judge: a hung job
holding a runner for GitHub's six-hour default, a runner image (and so the
lavapipe driver under the GPU tests) moving with nothing in any diff, or a
`ready_for_review` run cancelling a run of the very same commit. The last one
ties together the one list written in three places: which Markdown is code
because tests parse it (`docs/api/`, #525, #587).
"""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

from merging.queue import update
from merging.verdict import changes

SCRIPTS = Path(__file__).resolve().parents[1]
WORKFLOW_DIR = SCRIPTS.parent / "workflows"
GATING = ("ci.yml", "lint.yml")


#: A gate proves its commit on a fixed proptest seed; the nightly explores (#1800).
SEED_POLICY = r"(?m)^  PROPTEST_RNG_SEED: \$\{\{ github\.event_name != 'schedule' && '(?P<seed>\d+)' \|\| '' \}\}$"
#: The verbs that run proptests and must run them on that seed (#1852).
PINNED = ("test", "test-one-process")
#: Made a prerequisite of the verb, so it runs with that verb's variables and exports,
#: prints the seed its recipe would see, and stops make before the recipe runs.
PROBE = "{verb}: seed-probe\nseed-probe:\n\t@echo \"seed=$$PROPTEST_RNG_SEED\"; exit 3\n"


def jobs(workflow: str) -> dict[str, str]:
    """Each top-level job's name and its body, from the workflow's text."""
    text = (WORKFLOW_DIR / workflow).read_text().split("\njobs:\n", 1)[1]
    parts = re.split(r"^  ([\w-]+):\n", text, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


class ProptestSeed(unittest.TestCase):
    def test_the_gates_fix_it_and_the_nightly_leaves_it_random(self):
        self.assertRegex((WORKFLOW_DIR / "ci.yml").read_text(), SEED_POLICY)

    def test_a_seed_with_no_nightly_or_no_number_is_flagged(self):
        for line in ("  PROPTEST_RNG_SEED: 1784", "  PROPTEST_RNG_SEED: ${{ github.event_name != 'schedule' && '' || '' }}"):
            with self.subTest(line=line):
                self.assertNotRegex(line, SEED_POLICY)

    def test_the_make_verbs_run_on_the_same_seed(self):
        # One number (#1852): the Makefile reads ci.yml's, it never copies it.
        for verb in PINNED:
            with self.subTest(verb=verb):
                self.assertEqual(seen(verb, ROOT / "Makefile"), ci_seed())

    def test_a_verb_without_the_seed_or_with_a_copy_is_flagged(self):
        line = "export PROPTEST_RNG_SEED ?= $(PROPTEST_SEED)\n"
        text = (ROOT / "Makefile").read_text()
        self.assertEqual(text.count(line), 1, "the export line moved: update this test")
        for broken in ("", line.replace("$(PROPTEST_SEED)", str(int(ci_seed()) + 1))):
            with self.subTest(line=broken), tempfile.TemporaryDirectory() as tmp:
                (Path(tmp) / "Makefile").write_text(text.replace(line, broken))
                self.assertNotEqual(seen("test", Path(tmp) / "Makefile"), ci_seed())


ROOT = SCRIPTS.parents[1]


def ci_seed() -> str:
    return re.search(SEED_POLICY, (WORKFLOW_DIR / "ci.yml").read_text())["seed"]


def seen(verb: str, makefile: Path) -> str:
    """The PROPTEST_RNG_SEED `make <verb>`'s recipe runs with, from the Makefile alone:
    none is inherited (CI's jobs export one), nor the caller's make flags (#1852)."""
    drop = {"PROPTEST_RNG_SEED", "MAKEFLAGS", "MFLAGS", "MAKELEVEL"}
    env = {k: v for k, v in os.environ.items() if k not in drop}
    with tempfile.TemporaryDirectory() as tmp:
        probe = Path(tmp) / "probe.mk"
        probe.write_text(PROBE.format(verb=verb))
        out = subprocess.run(
            ["make", "-s", "--no-print-directory", "-f", str(makefile), "-f", str(probe), verb],
            cwd=ROOT, env=env, capture_output=True, text=True,
        ).stdout
    return "".join(l.removeprefix("seed=") for l in out.splitlines() if l.startswith("seed="))

class Runners(unittest.TestCase):
    def test_every_gating_job_has_a_timeout(self):
        for workflow in GATING:
            for name, body in jobs(workflow).items():
                with self.subTest(workflow=workflow, job=name):
                    self.assertRegex(body, r"(?m)^    timeout-minutes: \d+$")

    def test_runner_images_are_pinned(self):
        for workflow in GATING:
            text = (WORKFLOW_DIR / workflow).read_text()
            with self.subTest(workflow=workflow):
                self.assertNotIn("-latest", text)

    def test_no_dead_merge_group_trigger(self):
        # GitHub's native merge queue is not enabled; `make queue` is the queue.
        for workflow in GATING:
            with self.subTest(workflow=workflow):
                self.assertNotIn("merge_group", (WORKFLOW_DIR / workflow).read_text())


class Concurrency(unittest.TestCase):
    def test_only_a_push_to_a_ready_pr_cancels(self):
        for workflow in GATING:
            text = (WORKFLOW_DIR / workflow).read_text()
            block = text.split("\nconcurrency:\n", 1)[1].split("\n\n", 1)[0]
            with self.subTest(workflow=workflow):
                self.assertIn("!github.event.pull_request.draft", block)
                self.assertIn("github.run_id", block)
                self.assertIn("github.event.action == 'synchronize'", block)


class DocsThatAreCode(unittest.TestCase):
    """`CODE_DOCS` are code in ci.yml's path filter, `mergeable`, and the queue."""

    def test_ci_path_filter_reincludes_exactly_the_code_docs(self):
        body = jobs("ci.yml")["changes"]
        reincluded = re.findall(r"^\s+- '([^'!][^']*)'$", body, flags=re.M)
        self.assertEqual(sorted(reincluded), sorted(map(changes.code_doc_filter, changes.CODE_DOCS)))

    def test_mergeable_counts_them_as_code(self):
        for prefix in changes.CODE_DOCS:
            path = changes.code_doc_example(prefix)
            with self.subTest(path=path):
                self.assertFalse(changes.inert([path]))
                self.assertTrue(changes.needs_drift_run([path]))

    def test_the_queue_rebases_over_them(self):
        for prefix in changes.CODE_DOCS:
            with self.subTest(prefix=prefix):
                self.assertFalse(update.docs_advance(f">abc\n{changes.code_doc_example(prefix)}\n"))
        self.assertTrue(update.docs_advance(">abc\nREADME.md\n"))


if __name__ == "__main__":
    unittest.main()
