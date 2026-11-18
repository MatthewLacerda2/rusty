#!/usr/bin/env python3
"""Only a run that builds the engine saves the mutation build cache.

`mutants-on-request.yml`'s `rust-cache` step saves the `mutants` key from a
run on `main`, shard 0 alone, and rust-cache never re-saves a key it hit
exactly. A run whose plan holds only a standalone part (`tools/lint`:
`in_scope` 0, Baseline and Mutate skipped) builds no root `target/`, so a save
from it would park `~/.cargo` alone under the key and every later run would
hit it and build cold. These tests hold the `save-if` expression to that.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[3] / "workflows" / "mutants-on-request.yml"


def save_if(text: str) -> str:
    """The `save-if` expression of the step under `shared-key: mutants`."""
    found = re.search(r"shared-key: mutants\n\s+save-if: \$\{\{ (.+) \}\}\n", text)
    assert found, "no `save-if` under `shared-key: mutants` in mutants-on-request.yml"
    return found[1]


def evaluate(expr: str, **context: str) -> bool:
    """A conjunction of `name == value` / `name != value` terms, as the step writes it."""
    result = True
    for term in expr.split(" && "):
        name, op, value = re.fullmatch(r"([\w.]+) (==|!=) '?([^']*)'?", term.strip()).groups()
        equal = context[name] == value
        result = result and (equal if op == "==" else not equal)
    return result


def saves(ref: str = "refs/heads/main", shard: str = "0", in_scope: str = "5") -> bool:
    return evaluate(save_if(WORKFLOW.read_text()), **{
        "github.ref": ref, "matrix.shard": shard, "needs.plan.outputs.in_scope": in_scope})


class SaveIf(unittest.TestCase):
    def test_shard_zero_on_main_with_engine_mutants_saves(self) -> None:
        self.assertTrue(saves())

    def test_a_standalone_only_plan_does_not_save(self) -> None:
        self.assertFalse(saves(in_scope="0"))

    def test_another_shard_does_not_save(self) -> None:
        self.assertFalse(saves(shard="1"))

    def test_a_branch_run_does_not_save(self) -> None:
        self.assertFalse(saves(ref="refs/heads/2369-retire-mutation-sweep"))


if __name__ == "__main__":
    unittest.main()
