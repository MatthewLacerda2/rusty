#!/usr/bin/env python3
"""Each mutation run's own baseline runs exactly what its mutants run (#2164).

cargo-mutants 27 tests its baseline with `--package` set to the mutated
packages only, whatever `test_workspace` says, while each mutant tests the whole
workspace; and arguments after `--` reach only the test phase. So the workflows
skip cargo-mutants' baseline (`--baseline skip`), run their own `Baseline` step
and pass its timeout in. This holds the two together: the step's build and test
commands are the ones cargo-mutants will run for a mutant, the features go to
cargo-mutants (so every phase has them), and the mutants reuse the baseline's
`target/` (`--in-place`).
"""

from __future__ import annotations

import re
import unittest

from tests import SCRIPTS

WORKFLOWS = SCRIPTS.parent / "workflows"
#: A `cargo mutants` run (not `--list`) and its `\`-continued lines.
MUTATE = re.compile(r"cargo mutants (?!--list\b)(?:[^\n]*\\\n)*[^\n]*")


def commands(text: str) -> tuple[str, list[str]]:
    """The joined `cargo mutants` invocation and the baseline's nextest lines."""
    found = MUTATE.search(text)
    if not found:
        raise AssertionError("no `cargo mutants` run")
    mutate = " ".join(found[0].replace("\\\n", " ").split())
    step = text.split("- name: Baseline", 1)[1].split("\n      - name:", 1)[0] if "- name: Baseline" in text else ""
    return mutate, [" ".join(line.split()) for line in re.findall(r"cargo nextest run[^|\n]*", step)]


def problems(text: str) -> list[str]:
    """What keeps the workflow's baseline from being its mutants' baseline."""
    mutate, baseline = commands(text)
    own, _, test_args = mutate.partition(" -- ")
    test_args = test_args.split(" || ")[0]
    bad = [f"`cargo mutants` lacks `{flag}`" for flag in ("--baseline skip", "--in-place", "--timeout")
           if flag not in own]
    if "--features" in test_args:
        bad.append("`--features` after `--` reaches only the test phase; pass it to cargo-mutants")
    feats = " ".join(f"--features={f}" for f in re.findall(r"--features[ =](\S+)", own))
    packages = "--workspace" if "--workspace" in own else ""
    build = " ".join(f"cargo nextest run --no-run --verbose {packages} {feats}".split())
    test = " ".join(f"cargo nextest run --verbose {packages} {feats} {test_args}".split())
    for want in (build, test):
        if want not in baseline:
            bad.append(f"the `Baseline` step does not run `{want}` (it runs {baseline or 'nothing'})")
    return bad


class Workflows(unittest.TestCase):
    def test_both_mutation_workflows_run_their_mutants_baseline(self) -> None:
        for name in ("mutants-on-request.yml", "mutants-sweep.yml"):
            with self.subTest(name):
                self.assertEqual(problems((WORKFLOWS / name).read_text()), [])


GOOD = """\
      - name: Baseline
        run: |
          cargo nextest run --no-run --verbose --workspace --features=dev
          cargo nextest run --verbose --workspace --features=dev --fail-fast --test-threads 2
      - name: Mutate
        run: |
          cargo mutants --workspace --test-tool nextest --baseline skip --in-place \\
            --timeout "$T" --features dev \\
            --in-diff scope.diff -- --fail-fast --test-threads 2
"""


class Cases(unittest.TestCase):
    def test_a_matching_baseline_passes(self) -> None:
        self.assertEqual(problems(GOOD), [])

    def test_features_after_the_separator_are_flagged(self) -> None:
        text = GOOD.replace("--features dev \\", "\\").replace("-- --fail-fast", "-- --features dev --fail-fast")
        self.assertTrue(any("after `--`" in p for p in problems(text)))

    def test_cargo_mutants_own_baseline_is_flagged(self) -> None:
        self.assertIn("`cargo mutants` lacks `--baseline skip`",
                      problems(GOOD.replace("--baseline skip ", "")))

    def test_a_baseline_with_other_test_args_is_flagged(self) -> None:
        text = GOOD.replace("--test-threads 2\n      - name: Mutate", "--test-threads 4\n      - name: Mutate")
        self.assertTrue(any("does not run" in p for p in problems(text)))


if __name__ == "__main__":
    unittest.main()
