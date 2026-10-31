#!/usr/bin/env python3
"""The drift check flags a live ruleset short of the file and passes one that holds it."""

from __future__ import annotations

import contextlib
import io
import json
import unittest

import ruleset
from tests.ruleset.fixtures import AFTER, BEFORE, CHECKS, FakeGh

WANT = json.loads(ruleset.FILE.read_text())


def run(gh: FakeGh, *argv: str) -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        code = ruleset.main(list(argv), gh=ruleset.Gh(gh), repo="o/r")
    return code, out.getvalue()


class Drift(unittest.TestCase):
    def test_main_before_491_is_drift(self):
        # The case it must flag: no required checks, so GitHub merges a red PR.
        self.assertEqual(ruleset.drift(WANT, BEFORE), ["rule `required_status_checks` is missing"])

    def test_the_applied_file_is_not_drift(self):
        # The case it must pass: checks in another order, a key GitHub added.
        self.assertEqual(ruleset.drift(WANT, AFTER), [])

    def test_strict_mode_is_drift(self):
        strict = {**CHECKS, "parameters": {**CHECKS["parameters"], "strict_required_status_checks_policy": True}}
        gaps = ruleset.drift(WANT, {**BEFORE, "rules": [*BEFORE["rules"], strict]})
        self.assertEqual(len(gaps), 1)
        self.assertIn("rule `required_status_checks`", gaps[0])

    def test_a_check_from_any_app_is_drift(self):
        loose = {"context": "ci-gate"}, {"context": "lint-gate", "integration_id": 15368}
        params = {**CHECKS["parameters"], "required_status_checks": list(loose)}
        self.assertTrue(ruleset.drift(WANT, {**AFTER, "rules": [*BEFORE["rules"], {**CHECKS, "parameters": params}]}))

    def test_a_rule_only_github_has_is_drift(self):
        extra = {**AFTER, "rules": [*AFTER["rules"], {"type": "required_linear_history"}]}
        self.assertEqual(ruleset.drift(WANT, extra), ["rule `required_linear_history` is live but not committed"])

    def test_a_disabled_ruleset_is_drift(self):
        self.assertIn("`enforcement`", ruleset.drift(WANT, {**AFTER, "enforcement": "evaluate"})[0])

    def test_no_ruleset_is_drift(self):
        self.assertEqual(ruleset.drift(WANT, None), ["no ruleset named `main` on GitHub"])


class Command(unittest.TestCase):
    def test_the_default_reads_only(self):
        gh = FakeGh(BEFORE)
        code, out = run(gh)
        self.assertEqual((code, gh.writes), (1, []))
        self.assertIn("drift: rule `required_status_checks` is missing", out)

    def test_apply_puts_the_file_over_the_live_ruleset_and_checks_again(self):
        gh = FakeGh(BEFORE, after_write=AFTER)
        code, out = run(gh, "--apply")
        self.assertEqual(code, 0, out)
        self.assertEqual(gh.writes, [("PUT", "repos/o/r/rulesets/17663158", WANT)])

    def test_apply_creates_a_missing_ruleset(self):
        gh = FakeGh(None, after_write=AFTER)
        self.assertEqual(run(gh, "--apply")[0], 0)
        self.assertEqual([w[:2] for w in gh.writes], [("POST", "repos/o/r/rulesets")])

    def test_apply_writes_nothing_when_nothing_drifted(self):
        gh = FakeGh(AFTER)
        self.assertEqual(run(gh, "--apply")[0], 0)
        self.assertEqual(gh.writes, [])


if __name__ == "__main__":
    unittest.main()
