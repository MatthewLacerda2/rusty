#!/usr/bin/env python3
"""#913: `make reclaim` removes only finished worktrees, and never unsafe ones.

git and gh are faked by an injected runner that answers argv from tables, and
the disk effects (`free`, `remove`, `target_size`) are replaced, so nothing
here touches a real worktree.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import subprocess
import sys
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "reclaim.py"
_spec = importlib.util.spec_from_file_location("reclaim", SCRIPT)
reclaim = importlib.util.module_from_spec(_spec)
sys.modules["reclaim"] = reclaim  # dataclasses look their module up while loading
_spec.loader.exec_module(reclaim)

Worktree, Facts = reclaim.Worktree, reclaim.Facts
MAIN = Path("/repo")
TREES = MAIN / ".claude" / "worktrees"

PORCELAIN = f"""worktree {MAIN}
HEAD aaaa
branch refs/heads/main

worktree {TREES}/merged
HEAD m111
branch refs/heads/1-merged

worktree {TREES}/open
HEAD o111
branch refs/heads/2-open

worktree {TREES}/dirty
HEAD d111
branch refs/heads/3-dirty

worktree {TREES}/agent
HEAD aaaa
branch refs/heads/worktree-agent
locked claude agent (pid 1)

worktree {TREES}/queue
HEAD q111
detached

worktree /elsewhere/merged-too
HEAD e111
branch refs/heads/4-elsewhere
"""

PRS = {"1-merged": "MERGED", "2-open": "OPEN", "3-dirty": "MERGED", "4-elsewhere": "MERGED"}


class FakeMachine(reclaim.Machine):
    """Answers git/gh from the tables above and records what it would delete."""

    def __init__(self):
        super().__init__(self.answer)
        self.removed: list[str] = []
        self.calls: list[list[str]] = []

    def answer(self, argv, **_):
        self.calls.append(argv)
        out, code = "", 0
        if argv[0] == "gh":
            branch = argv[argv.index("--head") + 1]
            out = json.dumps([{"state": PRS[branch], "headRefOid": "x"}] if branch in PRS else [])
        elif "--git-common-dir" in argv:
            out = f"{MAIN}/.git"
        elif argv[3:5] == ["worktree", "list"]:
            out = PORCELAIN
        elif argv[3:5] == ["status", "--porcelain"]:
            out = " M src/lib.rs" if argv[2].endswith("dirty") else ""
        elif "--is-ancestor" in argv:
            code = 0  # every detached HEAD here is on origin/main
        elif argv[3] == "rev-list":
            out = "0"
        return subprocess.CompletedProcess(argv, code, out, "")

    def cwds(self):
        return [(42, TREES / "queue" / "sub"), (7, Path("/tmp"))]

    def free(self, path):
        return 10 * 2**30

    def target_size(self, wt):
        return 3 * 2**30

    def remove(self, main, wt):
        self.removed.append(wt.name)


def run(*argv: str) -> tuple[FakeMachine, str]:
    machine, out = FakeMachine(), io.StringIO()
    with contextlib.redirect_stdout(out):
        reclaim.main(list(argv), machine, here=TREES / "open")
    return machine, out.getvalue()


class Parse(unittest.TestCase):
    def test_reads_branches_detached_heads_and_locks(self):
        trees = reclaim.parse_worktrees(PORCELAIN + "\nworktree /d\nHEAD dddd\ndetached\n")
        self.assertEqual([t.branch for t in trees][:2], ["main", "1-merged"])
        self.assertEqual(trees[4].locked, "claude agent (pid 1)")
        self.assertIsNone(trees[1].locked)
        self.assertIsNone(trees[-1].branch)
        self.assertEqual(trees[-1].head, "dddd")


class Verdict(unittest.TestCase):
    wt = Worktree(Path("/w"), "h", "9-x")

    def test_pull_request_state_decides_a_branch(self):
        for prs, action in [(["MERGED"], "remove"), (["CLOSED"], "remove"), (["OPEN"], "keep"),
                            (["CLOSED", "OPEN"], "keep"), ([], "keep"), (None, "keep")]:
            self.assertEqual(reclaim.verdict(self.wt, Facts(prs=prs))[0], action, prs)

    def test_ancestry_decides_a_detached_head(self):
        detached = Worktree(Path("/w"), "h")
        self.assertEqual(reclaim.verdict(detached, Facts(on_main=True))[0], "remove")
        self.assertEqual(reclaim.verdict(detached, Facts(on_main=False))[0], "keep")

    def test_finished_but_unsafe_is_protected_and_named(self):
        locked = Worktree(Path("/w"), "h", "9-x", locked="")
        for wt, facts, says in [(self.wt, Facts(prs=["MERGED"], dirty=True), "uncommitted"),
                                (self.wt, Facts(prs=["MERGED"], unpushed=2), "2 commit(s)"),
                                (self.wt, Facts(prs=["MERGED"], current=True), "runs from"),
                                (self.wt, Facts(prs=["MERGED"], busy=[42]), "pid 42"),
                                (self.wt, Facts(prs=["MERGED"], busy=None), "no way to tell"),
                                (locked, Facts(prs=["MERGED"]), "locked")]:
            action, why = reclaim.verdict(wt, facts)
            self.assertEqual(action, "protect")
            self.assertIn(says, why)


class Cwds(unittest.TestCase):
    def test_reads_lsof_field_output_when_there_is_no_proc(self):
        if Path("/proc/self/cwd").exists():
            self.skipTest("Linux reads /proc, not lsof")
        lsof = "p12\nfcwd\nn/a/b\np13\nfcwd\nn/c\n"
        answer = lambda argv, **_: subprocess.CompletedProcess(argv, 1, lsof, "")
        machine = reclaim.Machine(answer)
        self.assertEqual(machine.cwds(), [(12, Path("/a/b")), (13, Path("/c"))])

    def test_no_lsof_output_is_unknown_not_empty(self):
        if Path("/proc/self/cwd").exists():
            self.skipTest("Linux reads /proc, not lsof")
        answer = lambda argv, **_: subprocess.CompletedProcess(argv, 1, "", "")
        self.assertIsNone(reclaim.Machine(answer).cwds())


class Unpushed(unittest.TestCase):
    def facts(self, stray: str, in_pr: bool) -> Facts:
        def answer(argv, **_):
            out = {"gh": json.dumps([{"state": "MERGED", "headRefOid": "pr"}])}.get(argv[0], "")
            if "rev-list" in argv:
                out = stray
            code = 1 if "--is-ancestor" in argv and not in_pr else 0
            return subprocess.CompletedProcess(argv, code, out, "")
        machine = reclaim.Machine(answer)
        return machine.facts(Worktree(Path("/w"), "h", "9-x"), Path("/repo"), cwds=[])

    def test_commits_stranded_by_a_deleted_remote_branch_are_safe_in_the_pr(self):
        self.assertEqual(self.facts("3", in_pr=True).unpushed, 0)

    def test_commits_nowhere_else_count_as_unpushed(self):
        self.assertEqual(self.facts("3", in_pr=False).unpushed, 3)


class Main(unittest.TestCase):
    def test_dry_run_names_everything_and_deletes_nothing(self):
        machine, out = run("--dry-run")
        self.assertEqual(machine.removed, [])
        self.assertNotIn(["git", "-C", str(MAIN), "worktree", "prune"], machine.calls)
        self.assertIn("would remove: merged (1-merged): pull request merged", out)
        self.assertIn("kept: open (2-open): pull request open", out)
        self.assertIn("PROTECTED: dirty (3-dirty)", out)
        self.assertIn("kept: agent (worktree-agent): no pull request", out)
        self.assertIn("target/ 3.0 GiB", out)
        self.assertIn("PROTECTED: queue (detached q111", out)
        self.assertIn("(pid 42)", out)
        self.assertNotIn("elsewhere", out)

    def test_removes_only_finished_safe_worktrees_then_prunes(self):
        machine, out = run()
        self.assertEqual(machine.removed, ["merged"])
        self.assertIn(["git", "-C", str(MAIN), "worktree", "prune"], machine.calls)
        self.assertIn("reclaim: removed 1 of 5 worktree(s); free disk 10.0 GiB -> 10.0 GiB", out)

    def test_unknown_argument_is_refused(self):
        with self.assertRaises(SystemExit):
            reclaim.main(["--force"], FakeMachine())


if __name__ == "__main__":
    unittest.main()
