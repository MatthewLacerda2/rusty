#!/usr/bin/env python3
"""#1482: local branches no worktree holds are deleted when finished, kept and named otherwise."""

from __future__ import annotations

import contextlib
import io
import json
import subprocess
import unittest
from pathlib import Path

import reclaim_branches as branches
from tests.reclaim.fixtures import reclaim, rest_pull

MAIN = Path("/repo")

# name: (tip, PR by name as (state, head) or None, PR by tip, commits on no remote)
BRANCHES = {
    "main": ("aaaa", None, None, "0"),
    "9-merged": ("m9", ("MERGED", "m9"), None, "2"),
    "pr1316-review": ("c1", None, ("CLOSED", "c1"), "1"),
    "10-open": ("o10", ("OPEN", "o10"), None, "1"),
    "scratch": ("s1", None, None, "3"),
    "11-closed": ("u11", ("CLOSED", "x11"), None, "1"),
    "12-here": ("h12", ("MERGED", "h12"), None, "0"),
    "fresh": ("aaaa", None, None, "0"),
}
ON_MAIN = {"aaaa"}


class Fake(reclaim.Machine):
    def __init__(self):
        super().__init__(self.answer)
        self.deleted: list[str] = []
        self.asked_by_tip: list[str] = []

    def answer(self, argv, **_):
        out, code = "", 0
        tips = {tip: name for name, (tip, *_) in BRANCHES.items()}
        if argv[0] == "gh" and "head=o:" in argv[2]:
            pr = BRANCHES[argv[2].split("head=o:")[1].split("&")[0]][1]
            out = json.dumps([rest_pull(*pr)] if pr else [])
        elif argv[0] == "gh":
            tip = argv[2].split("commits/")[1].split("/")[0]
            self.asked_by_tip.append(tip)
            pr = BRANCHES[tips[tip]][2]
            out = json.dumps([rest_pull(*pr)] if pr else [])
        elif "get-url" in argv:
            out = "https://github.com/o/r.git"
        elif "for-each-ref" in argv:
            out = "\n".join(f"{name} {tip}" for name, (tip, *_) in BRANCHES.items())
        elif "rev-list" in argv:
            out = BRANCHES[tips[argv[5]]][3]
        elif "--is-ancestor" in argv:
            commit, of = argv[-2:]
            code = 0 if commit == of or (of == "origin/main" and commit in ON_MAIN) else 1
        elif "merge-tree" in argv or "--version" in argv:
            code = 1
        return subprocess.CompletedProcess(argv, code, out, "")

    def delete_branch(self, main, branch):
        self.deleted.append(branch)


def visit(dry: bool = False) -> tuple[Fake, str]:
    machine, out = Fake(), io.StringIO()
    with contextlib.redirect_stdout(out):
        machine.count = branches.visit(machine, MAIN, {"12-here"}, dry, reclaim.judge_branch)
    return machine, out.getvalue()


class Deletes(unittest.TestCase):
    def test_a_merged_pr_s_branch_whose_commits_are_in_its_head(self):
        machine, out = visit()
        self.assertIn("9-merged", machine.deleted)
        self.assertIn("deleted branch: 9-merged: pull request merged", out)

    def test_a_reviewer_branch_found_by_its_tip_commit(self):
        machine, out = visit()
        self.assertIn("pr1316-review", machine.deleted)
        self.assertIn("deleted branch: pr1316-review: pull request closed", out)

    def test_only_those_two_and_counted(self):
        machine, _ = visit()
        self.assertEqual(sorted(machine.deleted), ["9-merged", "pr1316-review"])
        self.assertEqual(machine.count, 2)

    def test_dry_run_names_them_and_deletes_nothing(self):
        machine, out = visit(dry=True)
        self.assertEqual(machine.deleted, [])
        self.assertIn("would delete branch: 9-merged", out)
        self.assertIn("would delete branch: pr1316-review", out)


class Keeps(unittest.TestCase):
    def setUp(self):
        self.machine, self.out = visit()

    def test_an_open_pr_s_branch(self):
        self.assertIn("kept branch: 10-open: pull request open", self.out)

    def test_a_branch_with_no_pr_and_its_own_commits(self):
        self.assertIn("kept branch: scratch: no pull request", self.out)

    def test_a_closed_pr_s_branch_with_a_commit_on_no_remote(self):
        self.assertIn("PROTECTED branch: 11-closed: pull request closed, but 1 commit(s) are on no remote", self.out)

    def test_main_and_a_checked_out_branch_are_never_judged(self):
        self.assertNotIn("branch: main", self.out)
        self.assertNotIn("12-here", self.out)

    def test_a_tip_on_main_is_not_asked_by_commit(self):
        self.assertIn("kept branch: fresh: no pull request", self.out)
        self.assertNotIn("aaaa", self.machine.asked_by_tip)


class Refused(unittest.TestCase):
    def test_git_refusing_a_delete_is_named_and_not_counted(self):
        machine, out = Fake(), io.StringIO()
        machine.delete_branch = lambda main, branch: "error: branch is checked out"
        with contextlib.redirect_stdout(out):
            self.assertEqual(branches.delete(machine, MAIN, "x", "pull request merged", dry=False), 0)
        self.assertIn("PROTECTED branch: x: pull request merged, but git refused", out.getvalue())


if __name__ == "__main__":
    unittest.main()
