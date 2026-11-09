"""The macOS job times out before the queue gives up on it (#1976).

A job past its `timeout-minutes` is a cancelled run, which the queue re-runs
(#869); a job still running at the queue's `DEADLINE_MINUTES` is a stall, which
it hands back. So `build-test-cross`'s timeout must stay below that deadline,
and raising either one past the other fails here.
"""

from __future__ import annotations

import re
import unittest

from merging.queue import cli
from tests import SCRIPTS


def timeout(workflow: str, job: str) -> int:
    """`job`'s `timeout-minutes` in the workflow text."""
    start = workflow.index(f"\n  {job}:\n")
    end = re.search(r"\n  \S", workflow[start + 1:])
    body = workflow[start:start + 1 + end.start()] if end else workflow[start:]
    return int(re.search(r"\n    timeout-minutes: (\d+)\b", body).group(1))


class MacosTimeout(unittest.TestCase):
    def test_the_macos_job_times_out_before_the_queue_deadline(self):
        ci = (SCRIPTS.parent / "workflows" / "ci.yml").read_text(encoding="utf-8")
        self.assertLess(timeout(ci, "build-test-cross"), cli.DEADLINE_MINUTES)

    def test_a_timeout_is_read_from_its_own_job(self):
        text = "jobs:\n  a:\n    timeout-minutes: 10\n  b:\n    timeout-minutes: 50\n"
        self.assertEqual(timeout(text, "a"), 10)
        self.assertEqual(timeout(text, "b"), 50)
        self.assertFalse(timeout(text, "b") < cli.DEADLINE_MINUTES)


if __name__ == "__main__":
    unittest.main()
