#!/usr/bin/env python3
"""`make mutants-remote` never prints silence as a clean result (#750)."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "mutants-remote.py"
_spec = importlib.util.spec_from_file_location("mutants_remote", SCRIPT)
remote = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(remote)


class Unpushed(unittest.TestCase):
    def test_a_pushed_head_goes_ahead(self) -> None:
        self.assertIsNone(remote.unpushed("750-x", "abc", "abc"))

    def test_the_runner_would_mutate_other_code(self) -> None:
        self.assertIn("push", remote.unpushed("750-x", "abc", "def"))
        self.assertIn("does not exist", remote.unpushed("750-x", "abc", None))
        self.assertIn("detached", remote.unpushed("HEAD", "abc", "abc"))


class Find(unittest.TestCase):
    RUNS = [{"displayTitle": "mutants: diff [aaaa1111]"},
            {"displayTitle": "mutants: src/** [bbbb2222]"}]

    def test_finds_its_own_run_by_request_id(self) -> None:
        self.assertEqual(remote.find(self.RUNS, "bbbb2222"), self.RUNS[1])

    def test_another_request_on_the_branch_is_not_it(self) -> None:
        self.assertIsNone(remote.find(self.RUNS, "cccc3333"))


class Verdict(unittest.TestCase):
    def test_a_green_run_with_its_report_is_read(self) -> None:
        self.assertTrue(remote.verdict({"conclusion": "success"}, True)[0])

    def test_no_report_is_never_zero_survivors(self) -> None:
        for conclusion in ("failure", "cancelled", "timed_out", None):
            ok, sentence = remote.verdict({"conclusion": conclusion}, False)
            self.assertFalse(ok)
            self.assertIn("not a clean result", sentence)
        ok, sentence = remote.verdict({"conclusion": "success"}, False)
        self.assertFalse(ok)
        self.assertIn("no mutants-report artifact", sentence)


if __name__ == "__main__":
    unittest.main()
