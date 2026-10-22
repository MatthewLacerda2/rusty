#!/usr/bin/env python3
"""#964: every `make gates` gate runs in CI with the same command.

Today's Makefile and workflows must pass; a gate dropped from the YAML, a gate
with no command, and a stray clippy flag must each be flagged.
"""

from __future__ import annotations

import unittest

import gate_mirror

MAKEFILE = """\
LINT := cargo run --quiet --manifest-path tools/lint/Cargo.toml
QUICK_GATES := size
HEAVY_GATES := clippy doc deny
GATES := $(QUICK_GATES) $(HEAVY_GATES)

size: ## [gate] size
\t$(LINT)

clippy: ## [gate] clippy
\tcargo clippy --all-targets --features dev -- -D warnings

doc: ## [gate] doc
\tRUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked

deny: ## [gate] deny
\t@command -v cargo-deny >/dev/null 2>&1 || { \\
\t\techo "not installed"; exit 1; }
\tcargo deny check bans
"""

WORKFLOW = """\
jobs:
  lint:
    steps:
      - uses: actions/checkout@v7
      - name: Size
        run: cargo run --manifest-path tools/lint/Cargo.toml --verbose
      - name: Clippy
        run: |
          cargo clippy --features dev --all-targets -- -D warnings
      - name: Rustdoc
        env:
          RUSTDOCFLAGS: -D warnings
        run: cargo doc --locked --no-deps
      - uses: EmbarkStudios/cargo-deny-action@v2
        with:
          command: check bans
"""


def gaps(makefile: str = MAKEFILE, workflow: str = WORKFLOW) -> list[tuple[str, str]]:
    return gate_mirror.missing(
        gate_mirror.makefile_gates(makefile), gate_mirror.workflow_commands(workflow))


class Today(unittest.TestCase):
    def test_every_gate_command_runs_in_ci(self):
        gates, ci = gate_mirror.load()
        self.assertIn("deny", gates)
        self.assertEqual(gate_mirror.missing(gates, ci), [])

    def test_gates_are_exactly_quick_and_heavy_joined(self):
        text = (gate_mirror.ROOT / "Makefile").read_text()
        self.assertTrue(gate_mirror.gates_are_the_two_lists(text))
        self.assertFalse(gate_mirror.gates_are_the_two_lists(
            text.replace("GATES := $(QUICK_GATES) $(HEAVY_GATES)", "GATES := fmt $(HEAVY_GATES)")))

    def test_the_brief_names_every_heavy_gate(self):
        text = (gate_mirror.ROOT / "Makefile").read_text()
        brief = (gate_mirror.ROOT / ".claude/skills/issue-batch/cloud-brief.md").read_text()
        self.assertEqual(gate_mirror.brief_misses_heavy(text, brief), [])
        self.assertEqual(gate_mirror.brief_misses_heavy(text, brief.replace("make doc deny", "make deny")), ["doc"])

    def test_clippy_adds_only_deny_warnings(self):
        gates, ci = gate_mirror.load()
        local = [c for cmds in gates.values() for c in cmds]
        self.assertEqual(gate_mirror.clippy_extras(local + ci), [])


class Mirror(unittest.TestCase):
    def test_flag_order_env_steps_and_output_flags_pass(self):
        self.assertEqual(gaps(), [])

    def test_the_tool_presence_check_is_not_a_command(self):
        self.assertEqual(gate_mirror.makefile_gates(MAKEFILE)["deny"], ["cargo deny check bans"])

    def test_flags_a_gate_dropped_from_the_yaml(self):
        workflow = WORKFLOW.replace("cargo doc --locked --no-deps", "echo skipped")
        self.assertEqual([g for g, _ in gaps(workflow=workflow)], ["doc"])

    def test_flags_rustdocflags_missing_from_the_step(self):
        workflow = WORKFLOW.replace("        env:\n          RUSTDOCFLAGS: -D warnings\n", "")
        self.assertEqual([g for g, _ in gaps(workflow=workflow)], ["doc"])

    def test_flags_a_changed_flag(self):
        workflow = WORKFLOW.replace("--features dev --all-targets", "--all-targets")
        self.assertEqual([g for g, _ in gaps(workflow=workflow)], ["clippy"])

    def test_flags_a_gate_with_no_recipe(self):
        makefile = MAKEFILE.replace("QUICK_GATES := size", "QUICK_GATES := ghost size")
        self.assertEqual(gaps(makefile=makefile), [("ghost", "<no command>")])


class Clippy(unittest.TestCase):
    def test_flags_a_stray_lint_flag(self):
        stray = "cargo clippy --all-targets -- -D warnings -W clippy::pedantic"
        self.assertEqual(gate_mirror.clippy_extras([stray]), [stray])

    def test_flags_clippy_without_deny_warnings(self):
        self.assertEqual(gate_mirror.clippy_extras(["cargo clippy"]), ["cargo clippy"])

    def test_passes_deny_warnings_alone(self):
        self.assertEqual(gate_mirror.clippy_extras(["cargo clippy -- -D warnings"]), [])


if __name__ == "__main__":
    unittest.main()
