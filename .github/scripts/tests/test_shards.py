"""#2264: a suite split across a matrix runs every shard of it.

`gate_mirror` drops `--partition` when it compares CI with `make gates`, so this
is what keeps a split honest: today's workflows must pass, and a split missing a
shard, repeating one, or naming one past its count must each be flagged.
"""

from __future__ import annotations

import unittest

import gate_mirror


def entries(*shards: str) -> str:
    return "\n".join(f"          - {{ features: dev, shard: {s}, doctests: false }}" for s in shards)


class Shards(unittest.TestCase):
    def test_todays_workflows_run_every_shard(self):
        for name in gate_mirror.WORKFLOWS:
            with self.subTest(workflow=name):
                text = (gate_mirror.ROOT / ".github" / "workflows" / name).read_text()
                self.assertEqual(gate_mirror.shard_gaps(text), [])

    def test_a_whole_split_and_one_per_feature_set_pass(self):
        whole = entries("1/2", "2/2") + "\n          - { features: default, shard: 1/1, doctests: true }"
        self.assertEqual(gate_mirror.shard_gaps(whole), [])

    def test_flags_a_missing_repeated_or_out_of_range_shard(self):
        for shards in (("1/2",), ("1/2", "1/2"), ("1/2", "3/2"), ("1/3", "2/3")):
            with self.subTest(shards=shards):
                self.assertNotEqual(gate_mirror.shard_gaps(entries(*shards)), [])

    def test_the_partition_flag_is_neutral_in_the_mirror(self):
        sharded = 'cargo nextest run --locked --partition "count:${{ matrix.shard }}"'
        self.assertEqual(gate_mirror.normalise(sharded), gate_mirror.normalise("cargo nextest run --locked"))


if __name__ == "__main__":
    unittest.main()
