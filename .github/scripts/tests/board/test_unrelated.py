#!/usr/bin/env python3
"""A branch with no common ancestor with main is not audited, and the board says so.

GitHub's compare answers 404 "No common ancestor" for such a branch; the board
used to exit 2 on it. Any other failed call still exits 2.
"""

from __future__ import annotations

import contextlib
import io
import json
import subprocess
import unittest

from tests.board.test_board import NOW, board

LONE = "repos/o/r/compare/main...lone"
NO_ANCESTOR = "gh: No common ancestor between main and lone. (HTTP 404)"


def fake(answers: dict, broken: dict):
    def gh(argv, **_):
        path = argv[2].split("&per_page=")[0].split("?per_page=")[0]
        if path in broken:
            return subprocess.CompletedProcess(argv, 1, "", broken[path])
        return subprocess.CompletedProcess(argv, 0, json.dumps(answers[path]), "")
    return board.Gh(gh)


def run(branches: list[str], broken: dict) -> tuple[int, str, str]:
    answers = {"repos/o/r/branches": [{"name": n} for n in branches],
               "repos/o/r/issues?state=open": [], "repos/o/r/pulls?state=open": []}
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = board.main([], fake(answers, broken), repo="o/r", now=NOW)
    return code, out.getvalue(), err.getvalue()


class Unrelated(unittest.TestCase):
    def test_a_branch_sharing_no_history_is_skipped_and_said(self):
        code, out, err = run(["main", "lone"], {LONE: NO_ANCESTOR})
        self.assertEqual(code, 0)
        self.assertIn("Nothing stale: 1 branch(es)", out)
        self.assertEqual(err.count("not auditing lone: it shares no history with main"), 1)

    def test_any_other_failed_compare_still_exits_2(self):
        with self.assertRaises(SystemExit) as stop, contextlib.redirect_stderr(io.StringIO()):
            run(["main", "lone"], {LONE: "gh: Not Found (HTTP 404)"})
        self.assertEqual(stop.exception.code, 2)

    def test_the_lock_branch_is_kept_with_its_reason_and_never_compared(self):
        self.assertIn("orchestrator-lock", board.KEEP)
        code, _, err = run(["main", "orchestrator-lock"], {})  # a compare would KeyError
        self.assertEqual(code, 0)
        self.assertIn("keeping orchestrator-lock: the one-orchestrator lease", err)


if __name__ == "__main__":
    unittest.main()
