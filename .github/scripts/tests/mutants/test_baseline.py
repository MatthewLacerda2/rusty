#!/usr/bin/env python3
"""Each mutation run's own baseline runs exactly what its mutants run (#2164).

cargo-mutants 27 tests its baseline with `--package` set to the mutated
packages only, whatever `test_workspace` says, while each mutant tests what
`test_workspace` selects; and arguments after `--` reach only the test phase.
So the workflows skip cargo-mutants' baseline (`--baseline skip`), run their
own `Baseline` step and pass its timeout in. This holds the two together, by
building each phase's command the way cargo-mutants 27.1's `cargo_argv` does:
`--verbose`, the test packages (`--workspace` when `.cargo/mutants.toml`'s
`test_workspace` or `--test-workspace=true` says so; the CLI's own
`--workspace` only picks which packages are *mutated*), `--no-default-features`
and `--all-features`, every `--features` (the CLI's, then the config's), the
config's `additional_cargo_args`, and in the test phase only
`additional_cargo_test_args` and what follows `--`. It also holds that the
mutants' `--timeout` is the `Baseline` step's output, not a number, and that
the baseline runs under the same `ulimit -v` as the mutants it times.
"""

from __future__ import annotations

import re
import tomllib
import unittest

from tests import SCRIPTS

WORKFLOWS = SCRIPTS.parent / "workflows"
CONFIG = SCRIPTS.parent.parent / ".cargo" / "mutants.toml"
#: A `cargo mutants` run (not `--list`) and its `\`-continued lines.
MUTATE = re.compile(r"cargo mutants (?!--list\b)(?:[^\n]*\\\n)*[^\n]*")
ULIMIT = re.compile(r"ulimit -v (\d+)")


def step(text: str, name: str) -> str:
    """The body of the step whose name starts with `name`, or ''."""
    head = f"- name: {name}"
    return text.split(head, 1)[1].split("\n      - name:", 1)[0] if head in text else ""


def phases(own: str, test_args: str, config: dict) -> tuple[str, str]:
    """The build and test commands cargo-mutants runs for a mutant."""
    workspace = "--test-workspace=true" in own or (
        "--test-workspace=false" not in own and config.get("test_workspace") is True)
    parts = ["--verbose", "--workspace" if workspace else ""]
    parts += [flag for flag in ("--no-default-features", "--all-features")
              if flag in own or config.get(flag[2:].replace("-", "_"))]
    feats = re.findall(r"--features[ =](\S+)", own) + list(config.get("features", []))
    parts += [f"--features={f}" for f in feats]
    parts += re.findall(r"--cargo-arg[ =](\S+)", own) + list(config.get("additional_cargo_args", []))
    tail = " ".join(config.get("additional_cargo_test_args", [])) + " " + test_args
    common = " ".join(" ".join(parts).split())
    return (f"cargo nextest run --no-run {common}",
            " ".join(f"cargo nextest run {common} {tail}".split()))


def problems(text: str, config: dict) -> list[str]:
    """What keeps the workflow's baseline from being its mutants' baseline."""
    found = MUTATE.search(text)
    if not found:
        return ["no `cargo mutants` run"]
    mutate = " ".join(found[0].replace("\\\n", " ").split())
    own, _, test_args = mutate.partition(" -- ")
    test_args = test_args.split(" || ")[0]
    base = step(text, "Baseline")
    ran = [" ".join(line.split()) for line in re.findall(r"cargo nextest run[^|\n]*", base)]
    bad = [f"`cargo mutants` lacks `{flag}`" for flag in ("--baseline skip", "--in-place")
           if flag not in own]
    if "--features" in test_args:
        bad.append("`--features` after `--` reaches only the test phase; pass it to cargo-mutants")
    if not re.search(r'--timeout "?\$\{?TIMEOUT', own) or \
            "TIMEOUT: ${{ steps.baseline.outputs.timeout }}" not in step(text, "Mutat"):
        bad.append("`--timeout` must be `$TIMEOUT`, set from `steps.baseline.outputs.timeout`")
    if "id: baseline" not in base or "timeout=" not in base:
        bad.append("the `Baseline` step (`id: baseline`) must write `timeout=` to its output")
    caps = ULIMIT.findall(base), ULIMIT.findall(step(text, "Mutat"))
    if not caps[0] or caps[0] != caps[1]:
        bad.append(f"the `Baseline` step must run under the mutants' `ulimit -v` ({caps[1]}), not {caps[0]}")
    for want in phases(own, test_args, config):
        if want not in ran:
            bad.append(f"the `Baseline` step does not run `{want}` (it runs {ran or 'nothing'})")
    return bad


class Workflows(unittest.TestCase):
    def test_both_mutation_workflows_run_their_mutants_baseline(self) -> None:
        config = tomllib.loads(CONFIG.read_text())
        for name in ("mutants-on-request.yml", "mutants-sweep.yml"):
            with self.subTest(name):
                self.assertEqual(problems((WORKFLOWS / name).read_text(), config), [])


GOOD = """\
      - name: Baseline
        id: baseline
        run: |
          ulimit -v 8388608
          cargo nextest run --no-run --verbose --workspace --features=dev
          cargo nextest run --verbose --workspace --features=dev --fail-fast --test-threads 2
          echo "timeout=$t" >> "$GITHUB_OUTPUT"
      - name: Mutate
        env:
          TIMEOUT: ${{ steps.baseline.outputs.timeout }}
        run: |
          ulimit -v 8388608
          cargo mutants --workspace --test-tool nextest --baseline skip --in-place \\
            --timeout "$TIMEOUT" --features dev \\
            --in-diff scope.diff -- --fail-fast --test-threads 2
"""
WORKSPACE = {"test_workspace": True}


class Cases(unittest.TestCase):
    def test_a_matching_baseline_passes(self) -> None:
        self.assertEqual(problems(GOOD, WORKSPACE), [])

    def test_features_after_the_separator_are_flagged(self) -> None:
        text = GOOD.replace("--features dev \\", "\\").replace("-- --fail-fast", "-- --features dev --fail-fast")
        self.assertTrue(any("after `--`" in p for p in problems(text, WORKSPACE)))

    def test_cargo_mutants_own_baseline_is_flagged(self) -> None:
        self.assertIn("`cargo mutants` lacks `--baseline skip`",
                      problems(GOOD.replace("--baseline skip ", ""), WORKSPACE))

    def test_a_baseline_with_other_test_args_is_flagged(self) -> None:
        text = GOOD.replace("--test-threads 2\n          echo", "--test-threads 4\n          echo")
        self.assertTrue(any("does not run" in p for p in problems(text, WORKSPACE)))

    def test_dropping_test_workspace_is_flagged(self) -> None:
        """Mutants then test only their own package, which the baseline no longer times."""
        self.assertTrue(any("does not run `cargo nextest run --no-run --verbose --features=dev`" in p
                            for p in problems(GOOD, {})))

    def test_config_features_and_cargo_args_reach_the_baseline(self) -> None:
        config = {**WORKSPACE, "features": ["x"], "additional_cargo_args": ["--locked"]}
        self.assertTrue(any("--features=dev --features=x --locked" in p for p in problems(GOOD, config)))
        text = GOOD.replace("--features=dev", "--features=dev --features=x --locked")
        self.assertEqual(problems(text, config), [])

    def test_a_hard_coded_timeout_is_flagged(self) -> None:
        text = GOOD.replace('--timeout "$TIMEOUT"', "--timeout 300")
        self.assertTrue(any("`--timeout` must be" in p for p in problems(text, WORKSPACE)))

    def test_a_baseline_without_the_memory_cap_is_flagged(self) -> None:
        text = GOOD.replace("          ulimit -v 8388608\n          cargo nextest", "          cargo nextest", 1)
        self.assertTrue(any("`ulimit -v`" in p for p in problems(text, WORKSPACE)))


if __name__ == "__main__":
    unittest.main()
