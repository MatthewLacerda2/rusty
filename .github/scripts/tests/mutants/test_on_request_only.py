#!/usr/bin/env python3
"""Mutation runs only on request: no workflow runs `cargo mutants` on a schedule.

A scheduled sweep of whole modules held hundreds of runner-hours a week on the
runners the gating CI needs, never finished inside a weekend, and no follow-up
ever came from its report: every survivor issue came from a scoped run asked on
the pull request that wrote the code (#2369). So mutation is dispatched, over
what someone names (`mutants-on-request.yml`, `make mutants-remote`), and a
workflow that both has an `on.schedule` trigger and runs `cargo mutants` fails
here. Auditing a whole module stays a by-hand ask for its glob.
"""

from __future__ import annotations

import re
import unittest

from tests import SCRIPTS

WORKFLOWS = SCRIPTS.parent / "workflows"


def triggers(text: str) -> str:
    """The workflow's `on:` value: its inline value and its indented block."""
    lines = text.splitlines()
    for i, line in enumerate(lines):
        found = re.match(r"""^["']?on["']?:\s*(.*?)\s*(#.*)?$""", line)
        if not found:
            continue
        block = [found[1]]
        for body in lines[i + 1:]:
            if body.strip() and not body.startswith((" ", "#")):
                break
            block.append(body)
        return "\n".join(block)
    return ""


def scheduled_mutation(text: str) -> bool:
    """Whether a workflow's text is scheduled and runs `cargo mutants` outside a comment."""
    on = re.sub(r"#.*", "", triggers(text))
    scheduled = re.search(r"(^|[\s\[,])schedule\b", on) is not None
    code = "\n".join(re.sub(r"(^|\s)#.*$", "", line) for line in text.splitlines())
    return scheduled and re.search(r"\bcargo\s+mutants\b", code) is not None


class Workflows(unittest.TestCase):
    def test_no_workflow_mutates_on_a_schedule(self) -> None:
        paths = sorted(WORKFLOWS.glob("*.yml"))
        self.assertTrue(paths, f"no workflow found under {WORKFLOWS}")
        for path in paths:
            with self.subTest(path.name):
                self.assertFalse(scheduled_mutation(path.read_text()),
                                 f"{path.name} runs `cargo mutants` on a schedule: mutation runs only on "
                                 "request (`mutants-on-request.yml`, scoped to what someone names); drop "
                                 "the `schedule:` trigger or the mutation step (test_on_request_only.py says why)")


RUN = "jobs:\n  m:\n    steps:\n      - run: cargo mutants --workspace\n"


class Cases(unittest.TestCase):
    def test_a_scheduled_mutation_run_is_flagged(self) -> None:
        self.assertTrue(scheduled_mutation("on:\n  schedule:\n    - cron: '0 4 * * 6'\n" + RUN))
        self.assertTrue(scheduled_mutation("on: [schedule, workflow_dispatch]\n" + RUN))

    def test_a_dispatched_mutation_run_passes(self) -> None:
        self.assertFalse(scheduled_mutation("on:\n  workflow_dispatch:\n" + RUN))

    def test_a_scheduled_run_without_mutation_passes(self) -> None:
        text = "on:\n  schedule:\n    - cron: '0 4 * * 1'\njobs:\n  c:\n    steps:\n      - run: cargo nextest run\n"
        self.assertFalse(scheduled_mutation(text))
        self.assertFalse(scheduled_mutation(text + "      # cargo mutants ran here once\n"))

    def test_a_comment_naming_a_schedule_is_not_a_trigger(self) -> None:
        self.assertFalse(scheduled_mutation("on:\n  # no schedule: dispatched only\n  workflow_dispatch:\n" + RUN))


if __name__ == "__main__":
    unittest.main()
