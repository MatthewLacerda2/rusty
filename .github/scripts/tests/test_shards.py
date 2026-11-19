"""#2264: a suite split across a matrix runs every shard of it.

`gate_mirror` drops `--partition` when it compares CI with `make gates`, so this
is what keeps a split honest: today's workflows must pass; a split missing a
shard, repeating one or naming one past its count, a `--partition` that does
not follow the matrix, and one whose split it cannot read must each be flagged.
"""

from __future__ import annotations

import re
import unittest

import gate_mirror

FOLLOWS = 'cargo nextest run --locked --partition "count:${{ matrix.shard }}"'


def workflow(*shards: str, run: str = FOLLOWS, matrix: str | None = None) -> str:
    entries = "\n".join(f"          - {{ features: dev, shard: {s} }}" for s in shards)
    return ("jobs:\n  build-test:\n    strategy:\n      matrix:\n        include:\n"
            f"{matrix if matrix is not None else entries}\n    steps:\n"
            "      # nextest's `--partition` splits the suite\n"
            f"      - name: Test\n        run: {run}\n  other:\n    steps:\n      - run: make scripts\n")


class Shards(unittest.TestCase):
    def test_todays_workflows_run_every_shard(self):
        for name in gate_mirror.WORKFLOWS:
            with self.subTest(workflow=name):
                text = (gate_mirror.ROOT / ".github" / "workflows" / name).read_text()
                self.assertEqual(gate_mirror.shard_gaps(text), [])

    def test_a_whole_split_and_one_per_feature_set_pass(self):
        whole = workflow("1/2", "2/2").replace("dev, shard: 2/2 }", "dev, shard: 2/2 }\n          - { features: default, shard: 1/1 }")
        self.assertEqual(gate_mirror.shard_gaps(whole), [])
        self.assertEqual(gate_mirror.shard_gaps(workflow("1/1", run="cargo nextest run --partition=count:${{ matrix.shard }}")), [])

    def test_flags_a_missing_repeated_or_out_of_range_shard(self):
        for shards in (("1/2",), ("1/2", "1/2"), ("1/2", "3/2"), ("1/3", "2/3")):
            with self.subTest(shards=shards):
                self.assertNotEqual(gate_mirror.shard_gaps(workflow(*shards)), [])

    def test_flags_a_partition_that_does_not_follow_the_matrix(self):
        for run in ("cargo nextest run --partition count:1/2", "cargo nextest run --partition hash:${{ matrix.shard }}"):
            with self.subTest(run=run):
                self.assertNotEqual(gate_mirror.shard_gaps(workflow("1/2", "2/2", run=run)), [])

    def test_flags_a_partition_whose_split_it_cannot_read(self):
        block = "          - features: dev\n            shard: 1/2\n          - features: dev\n            shard: 2/2"
        self.assertNotEqual(gate_mirror.shard_gaps(workflow(matrix=block)), [])
        self.assertNotEqual(gate_mirror.shard_gaps(workflow(matrix="")), [])

    def test_a_split_by_mains_times_follows_the_matrix_or_is_flagged(self):
        planned = 'while read -r a; do s+=("$a"); done < <(python3 .github/scripts/shard_plan.py args "${{ matrix.shard }}")'
        self.assertEqual(gate_mirror.shard_gaps(workflow("1/2", "2/2", run=planned)), [])
        for shards, run in ((("1/2", "2/2"), planned.replace('"${{ matrix.shard }}"', "1/2")),
                            (("1/2",), planned), ((), planned)):
            with self.subTest(shards=shards, run=run):
                self.assertNotEqual(gate_mirror.shard_gaps(workflow(*shards, run=run)), [])

    def test_a_partition_in_a_comment_or_a_job_without_one_passes(self):
        self.assertEqual(gate_mirror.shard_gaps(workflow("1/2", "2/2", run="cargo nextest run --locked")), [])

    def test_the_partition_flag_is_neutral_in_the_mirror(self):
        self.assertEqual(gate_mirror.normalise(FOLLOWS), gate_mirror.normalise("cargo nextest run --locked"))


class Canaries(unittest.TestCase):
    """Each shard proves its own runner's GPU and OIDN: a `--partition` puts each
    canary in one shard, so every sharded job (`build-test`, and the macOS leg
    since #2276) runs the canary step on every entry, by names that exist."""

    def test_every_entry_runs_the_canaries_by_their_real_names(self):
        text = (gate_mirror.ROOT / ".github" / "workflows" / "ci.yml").read_text()
        sources = "\n".join(p.read_text() for p in (gate_mirror.ROOT / "crates").rglob("*.rs"))
        sharded = [(job, body) for job, body in gate_mirror._jobs(text) if "--partition" in body]
        self.assertTrue(sharded, "no job of ci.yml holds a `--partition`: this test would hold nothing")
        for job, body in sharded:
            with self.subTest(job=job):
                self.assertIn("      - name: Canaries", body, "a sharded job runs the canaries (#2276)")
                step = body[body.index("      - name: Canaries"):]
                step = step[: step.index("\n      - name:", 1)]
                self.assertNotIn("matrix.", step.split("run:", 1)[0], "the canaries run on every matrix entry")
                names = re.findall(r"test\(=([\w:]+)\)", step)
                self.assertEqual(len(names), 2, step)
                for name in names:
                    self.assertRegex(sources, rf"#\[test\]\s*fn {name.rsplit('::', 1)[1]}\(")


if __name__ == "__main__":
    unittest.main()
