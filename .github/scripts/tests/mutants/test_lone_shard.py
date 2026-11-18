#!/usr/bin/env python3
"""A sweep whose only report is one shard's still reports that shard (#2288).

`mutants-report` downloads the shards by `pattern:`, and `download-artifact`
unpacks a lone match flat into `shards/`. These tests run the report job's own
shell, lifted from the workflow, on that flat layout and on the usual one: the
merged lists, the kept rate files and the `sweep-complete` marker must come out
the same for one shard as for two.
"""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

from tests import load

ROOT = Path(__file__).resolve().parents[4]
artifact_dirs = load("artifact-dirs.py")
WORKFLOW = ROOT / ".github" / "workflows" / "mutants-sweep.yml"


def report_scripts() -> list[str]:
    """The shell of every `run:` step after `mutants-report`'s download, in order, dedented."""
    text = WORKFLOW.read_text()
    report = text.split("\n  mutants-report:\n", 1)[1]
    steps = next(block for block in artifact_dirs.jobs_steps("jobs:\n  r:\n" + report)
                 if any("download-artifact@" in step for step in block))
    after = steps[next(k for k, step in enumerate(steps) if "download-artifact@" in step) + 1:]
    scripts = []
    for step in after:
        if "\n        run: |\n" in step:
            body = step.split("\n        run: |\n", 1)[1]
            scripts.append("\n".join(line[10:] for line in body.splitlines()) + "\n")
        elif m := re.search(r"(?m)^ +(?:- )?run: (.+)$", step):
            scripts.append(m[1] + "\n")
    return scripts


def upload(shard: Path, k: int, total: int) -> None:
    """What one finished shard uploads: as `mutants` writes it, `artifact-name` included."""
    shard.mkdir(parents=True)
    (shard / "exit-code").write_text("0\n")
    (shard / "elapsed-seconds").write_text("3600\n")
    (shard / "shard-total").write_text(f"{total}\n")
    (shard / "artifact-name").write_text(f"mutants-shard-{k}\n")
    (shard / "caught.txt").write_text("".join(f"s{k}-m{i}\n" for i in range(12)))
    (shard / "missed.txt").write_text(f"s{k}-survivor\n")


class LoneShard(unittest.TestCase):
    def report(self, layout: str) -> tuple[Path, str]:
        """Run the split and merge steps on `layout` ('flat' or 'split'); (out dir, summary)."""
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(subprocess.run, ["rm", "-rf", str(tmp)])
        (tmp / ".github").symlink_to(ROOT / ".github")
        if layout == "flat":   # a lone match: its files straight in `shards/`
            upload(tmp / "shards", 0, 1)
        else:
            for k in range(2):
                upload(tmp / "shards" / f"mutants-shard-{k}", k, 2)
        summary = tmp / "summary"
        env = dict(os.environ, GITHUB_STEP_SUMMARY=str(summary), PLANNED="", SIZED_AT="")
        for script in report_scripts():
            done = subprocess.run(["bash", "-e", "-c", script], cwd=tmp, env=env,
                                  capture_output=True, text=True, check=False)
            self.assertEqual(done.returncode, 0, f"{script[:80]}: {done.stderr}")
        return tmp / "mutants.out", summary.read_text()

    def test_a_lone_shard_unpacked_flat_is_reported(self):
        out, summary = self.report("flat")
        self.assertEqual(len((out / "caught.txt").read_text().splitlines()), 12)
        self.assertEqual((out / "missed.txt").read_text(), "s0-survivor\n")
        self.assertTrue((out / "shards" / "mutants-shard-0" / "elapsed-seconds").is_file())
        self.assertEqual((out / "sweep-complete").read_text(), "1\n")
        self.assertIn("All 1 shards finished.", summary)

    def test_many_shards_are_reported_as_before(self):
        out, summary = self.report("split")
        self.assertEqual(len((out / "caught.txt").read_text().splitlines()), 24)
        self.assertEqual(sorted(p.name for p in (out / "shards").iterdir()),
                         ["mutants-shard-0", "mutants-shard-1"])
        self.assertIn("All 2 shards finished.", summary)

    def test_every_shard_names_its_artifact(self):
        # What the split reads must match what the upload is called.
        text = WORKFLOW.read_text()
        self.assertTrue("name: mutants-shard-${{ matrix.shard }}\n" in text, "the shard upload was renamed")
        self.assertTrue("echo 'mutants-shard-${{ matrix.shard }}' > mutants.out/artifact-name" in text,
                        "each shard must write its artifact's name to mutants.out/artifact-name")


if __name__ == "__main__":
    unittest.main()
