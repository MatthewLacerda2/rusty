#!/usr/bin/env python3
"""Mutation runs only on request: no scheduled workflow runs or dispatches a mutation run.

A scheduled sweep of whole modules held hundreds of runner-hours a week on the
runners the gating CI needs, never finished inside a weekend, and no follow-up
ever came from its report: every survivor issue came from a scoped run asked on
the pull request that wrote the code. So mutation is dispatched, over what
someone names (`mutants-on-request.yml`, `make mutants-remote`), and a workflow
with an `on.schedule` trigger fails here when its code names `cargo mutants`,
`cargo-mutants`, `mutants-on-request` or `mutants-remote`: running the tool,
dispatching the on-request run, or calling it as a reusable workflow. Auditing
a whole module stays a by-hand ask for its glob.

The scripts' tests carry no YAML reader, so this reads the text: comments are
stripped outside quotes, and the trigger is found in the `on:` key's inline
value (block, list or flow form) and its indented block.
"""

from __future__ import annotations

import re
import unittest

from tests import SCRIPTS

WORKFLOWS = SCRIPTS.parent / "workflows"
MUTATION = re.compile(r"\bcargo[\s-]+mutants\b|\bmutants-on-request\b|\bmutants-remote\b")


def uncomment(line: str) -> str:
    """The line without its YAML comment: a `#` after whitespace, outside quotes.

    A quote opens a string only where a scalar starts (after whitespace or `[{,:`),
    so an apostrophe inside a plain word (`don't`) opens nothing.
    """
    quote = ""
    for i, ch in enumerate(line):
        if quote:
            if ch == quote:
                quote = ""
        elif ch in "'\"" and (i == 0 or line[i - 1].isspace() or line[i - 1] in "[{,:"):
            quote = ch
        elif ch == "#" and (i == 0 or line[i - 1].isspace()):
            return line[:i]
    return line


def code(text: str) -> list[str]:
    return [uncomment(line).rstrip() for line in text.splitlines()]


def triggers(lines: list[str]) -> str:
    """The workflow's `on:` value: its inline value and its indented block."""
    for i, line in enumerate(lines):
        found = re.match(r"""^["']?on["']?\s*:\s*(.*)$""", line)
        if not found:
            continue
        block = [found[1]]
        for body in lines[i + 1:]:
            if body.strip() and not body.startswith(" "):
                break
            block.append(body)
        return "\n".join(block)
    return ""


def scheduled_mutation(text: str) -> bool:
    """Whether a workflow is scheduled and its code runs or dispatches mutation."""
    lines = code(text)
    scheduled = re.search(r"""(^|[\s\[{,'"])schedule\b""", triggers(lines)) is not None
    return scheduled and MUTATION.search("\n".join(lines)) is not None


class Workflows(unittest.TestCase):
    def test_no_workflow_mutates_on_a_schedule(self) -> None:
        paths = sorted([*WORKFLOWS.glob("*.yml"), *WORKFLOWS.glob("*.yaml")])
        self.assertTrue(paths, f"no workflow found under {WORKFLOWS}")
        for path in paths:
            with self.subTest(path.name):
                self.assertFalse(scheduled_mutation(path.read_text()),
                                 f"{path.name} runs or dispatches mutation on a schedule: mutation runs "
                                 "only on request (`mutants-on-request.yml`, scoped to what someone names); "
                                 "drop the `schedule:` trigger or the mutation step (test_on_request_only.py "
                                 "says why)")


SCHEDULE = "on:\n  schedule:\n    - cron: '0 4 * * 6'\n"
RUN = "jobs:\n  m:\n    steps:\n      - run: cargo mutants --workspace\n"


def step(run: str) -> str:
    return f"jobs:\n  m:\n    steps:\n      - run: {run}\n"


class Cases(unittest.TestCase):
    def test_a_scheduled_mutation_run_is_flagged(self) -> None:
        self.assertTrue(scheduled_mutation(SCHEDULE + RUN))
        self.assertTrue(scheduled_mutation("on: [schedule, workflow_dispatch]\n" + RUN))

    def test_the_flow_form_is_flagged(self) -> None:
        self.assertTrue(scheduled_mutation("on: {schedule: [{cron: '0 4 * * 6'}]}\n" + RUN))

    def test_dispatching_the_on_request_run_is_flagged(self) -> None:
        self.assertTrue(scheduled_mutation(SCHEDULE + step("gh workflow run mutants-on-request.yml -f scope=diff")))

    def test_make_mutants_remote_is_flagged(self) -> None:
        self.assertTrue(scheduled_mutation(SCHEDULE + step("make mutants-remote SCOPE='crates/physics/src/**'")))

    def test_calling_it_as_a_reusable_workflow_is_flagged(self) -> None:
        self.assertTrue(scheduled_mutation(SCHEDULE + "jobs:\n  m:\n    uses: ./.github/workflows/mutants-on-request.yml\n"))

    def test_the_hyphenated_tool_name_is_flagged(self) -> None:
        self.assertTrue(scheduled_mutation(SCHEDULE + step("cargo-mutants mutants --workspace")))

    def test_a_hash_inside_quotes_hides_nothing(self) -> None:
        self.assertTrue(scheduled_mutation(SCHEDULE + step("echo \"step #1\"; cargo mutants")))

    def test_an_apostrophe_opens_no_string(self) -> None:
        self.assertFalse(scheduled_mutation(SCHEDULE + step("echo don't  # cargo mutants")))

    def test_a_dispatched_mutation_run_passes(self) -> None:
        self.assertFalse(scheduled_mutation("on:\n  workflow_dispatch:\n" + RUN))

    def test_a_scheduled_run_without_mutation_passes(self) -> None:
        text = SCHEDULE + step("cargo nextest run")
        self.assertFalse(scheduled_mutation(text))
        self.assertFalse(scheduled_mutation(text + "      # cargo mutants ran here once (mutants-on-request.yml)\n"))

    def test_a_comment_naming_a_schedule_is_not_a_trigger(self) -> None:
        self.assertFalse(scheduled_mutation("on:\n  # no schedule: dispatched only\n  workflow_dispatch:\n" + RUN))


if __name__ == "__main__":
    unittest.main()
