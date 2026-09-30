#!/usr/bin/env python3
"""#587: `main` moving by Markdown alone costs a waiting branch nothing.

Merging a Markdown-only pull request while another waited on CI used to
rebase the waiting one and buy it a second CI round. These drive
[`queue.advance`] over a real throwaway repository, so the `git log` that
decides is the one the queue runs: a Markdown advance is neither rebased nor
pushed, a code advance is rebased, and `docs/scripting-api.md` counts as code.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).resolve().parent.parent / "merge-queue.py"
_spec = importlib.util.spec_from_file_location("merge_queue_md", SCRIPT)
queue = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(queue)
mergeable = queue.mergeable

# The rebase in [`queue.advance`] commits, and a CI runner has no git identity.
WHO = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}


class Repo:
    """A repository whose `origin/main` is ahead of a branch by chosen files."""

    def __init__(self, root: str):
        self.root = root
        self.git("init", "--quiet", "--initial-branch=main")
        self.commit({"src/lib.rs": "fn a() {}\n", "README.md": "# r\n", "docs/scripting-api.md": "# api\n"})
        self.base = self.sha()
        self.head = self.commit({"src/feature.rs": "fn f() {}\n"})

    def git(self, *args: str) -> str:
        return subprocess.run(["git", *args], cwd=self.root, env={**os.environ, **WHO}, capture_output=True, text=True, check=True).stdout

    def sha(self) -> str:
        return self.git("rev-parse", "HEAD").strip()

    def commit(self, files: dict[str, str]) -> str:
        for path, text in files.items():
            (Path(self.root) / path).parent.mkdir(parents=True, exist_ok=True)
            (Path(self.root) / path).write_text(text)
        self.git("add", "-A")
        self.git("commit", "--quiet", "-m", "c")
        return self.sha()

    def main_gains(self, *changes: dict[str, str]) -> None:
        """Put `origin/main` on the base plus these commits, then leave HEAD detached."""
        self.git("checkout", "--quiet", "--detach", self.base)
        for files in changes:
            self.commit(files)
        self.git("update-ref", "refs/remotes/origin/main", self.sha())


class Advancing(unittest.TestCase):
    def advance(self, *changes: dict[str, str]):
        with tempfile.TemporaryDirectory() as root:
            repo = Repo(root)
            repo.main_gains(*changes)
            verbs = []
            real = queue.git

            def spy(*args, cwd=None):
                verbs.append(args[0])
                return real(*args, cwd=cwd)

            check = mock.Mock(return_value=[])
            with mock.patch.object(queue, "git", side_effect=spy), mock.patch.dict(os.environ, WHO):
                # A push would fail loudly: there is no `origin` remote.
                fresh, notes = queue.advance("b", repo.head, root, push=False, check=check)
            return fresh != repo.head, notes, verbs, check

    def test_a_markdown_advance_is_not_rebased_pushed_or_checked(self):
        moved, notes, verbs, check = self.advance({"README.md": "# r\nmore\n"}, {"docs/new.md": "x\n"})
        self.assertFalse(moved)
        self.assertIn("only by Markdown", notes[0])
        self.assertNotIn("rebase", verbs)
        self.assertNotIn("push", verbs)
        check.assert_not_called()

    def test_a_code_advance_is_rebased_and_checked(self):
        moved, notes, verbs, check = self.advance({"README.md": "# r\nmore\n"}, {"src/lib.rs": "fn b() {}\n"})
        self.assertTrue(moved)
        self.assertEqual(notes, [])
        self.assertIn("rebase", verbs)
        check.assert_called_once()

    def test_the_api_doc_advance_is_code(self):
        moved, _, verbs, _ = self.advance({"docs/scripting-api.md": "# api\nTransform.x\n"})
        self.assertTrue(moved)
        self.assertIn("rebase", verbs)

    def test_a_rename_to_markdown_is_code(self):
        with tempfile.TemporaryDirectory() as root:
            repo = Repo(root)
            repo.git("checkout", "--quiet", "--detach", repo.base)
            repo.git("mv", "src/lib.rs", "src/lib.md")
            repo.git("commit", "--quiet", "-m", "c")
            repo.git("update-ref", "refs/remotes/origin/main", repo.sha())
            self.assertFalse(queue.main_moved_by_docs(repo.head, root))

    def test_a_head_already_on_main_is_not_an_advance(self):
        with tempfile.TemporaryDirectory() as root:
            repo = Repo(root)
            repo.git("update-ref", "refs/remotes/origin/main", repo.base)
            self.assertFalse(queue.main_moved_by_docs(repo.head, root))


class Deciding(unittest.TestCase):
    def test_the_log_is_read_commit_by_commit(self):
        self.assertTrue(queue.docs_advance(">abc\n\nREADME.md\n>def\n\ndocs/ui.md\n"))
        self.assertFalse(queue.docs_advance(">abc\n\nREADME.md\nsrc/app/mod.rs\n"))
        self.assertFalse(queue.docs_advance(">abc\n\ndocs/scripting-api.md\n"))
        self.assertFalse(queue.docs_advance(""))

    def test_an_empty_commit_changes_nothing(self):
        self.assertTrue(queue.docs_advance(">abc\n"))

    def test_a_git_failure_rebases_as_before(self):
        failed = subprocess.CompletedProcess([], 128, "", "fatal: bad revision")
        with mock.patch.object(queue, "git", return_value=failed):
            self.assertFalse(queue.main_moved_by_docs("0" * 40, "."))

    def test_dependabot_behind_only_by_markdown_is_not_asked_to_rebase(self):
        opts = queue.parse(["524"])
        with mock.patch.object(queue, "on_tip", return_value=False), \
            mock.patch.object(queue, "main_moved_by_docs", return_value=True), \
            mock.patch.object(queue.subprocess, "run") as called:
            self.assertEqual(queue.bot_head(524, "a" * 40, ".", opts), ("a" * 40, []))
        called.assert_not_called()

    def test_mergeable_accepts_a_green_head_behind_main(self):
        pull = {"isDraft": False, "headRefOid": "a" * 40, "mergeStateStatus": "BEHIND", "mergeable": "MERGEABLE"}
        runs = [{"name": w, "head_sha": "a" * 40, "id": i, "status": "completed", "conclusion": "success"}
                for i, w in enumerate(mergeable.WORKFLOWS)]
        jobs = {i: [{"name": n, "conclusion": "success"} for n in (g, *gated)]
                for i, (g, gated) in enumerate(mergeable.WORKFLOWS.values())}
        self.assertTrue(mergeable.judge(pull, runs, jobs, ["src/app/mod.rs"])[0])


if __name__ == "__main__":
    unittest.main()
