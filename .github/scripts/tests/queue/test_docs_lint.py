#!/usr/bin/env python3
"""A head `main` passed only by Markdown is not merged in, but when it touches an
agent doc its merged tree is still linted ([`update.docs_merge_check`]): two
Markdown-only branches can each fit `CLAUDE.md`'s cap and overflow it merged, and
the second is handed back before any push. Driven over a real throwaway
repository, with a stand-in for `tools/lint`'s size pass."""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from merging.queue import checks, update
from tests.queue.test_markdown_advance import WHO, Repo

CAP = 300


def over_cap(work: str) -> list[str]:
    """The size pass's verdict on `CLAUDE.md`, as `Linter.check` words a failure."""
    lines = len((Path(work) / "CLAUDE.md").read_text().splitlines())
    return [f"the merged tree fails tools/lint: CLAUDE.md:{CAP + 1}: [claude-md] {lines}/{CAP}; not pushed."] if lines > CAP else []


class DocsOnly(unittest.TestCase):
    def take(self, mine: int, theirs: int):
        """The head adds `mine` lines to a 200-line `CLAUDE.md`; `main` gained `theirs`."""
        with tempfile.TemporaryDirectory() as root:
            repo = Repo(root)
            base = "".join(f"rule {i}\n" for i in range(200))
            repo.git("checkout", "--quiet", "--detach", repo.head)
            repo.head = repo.commit({"CLAUDE.md": base})
            repo.base = repo.head
            repo.head = repo.commit({"CLAUDE.md": base + "mine\n" * mine})
            repo.main_gains({"CLAUDE.md": "theirs\n" * theirs + base})
            check = mock.Mock(side_effect=over_cap)
            with mock.patch.dict(os.environ, WHO):
                fresh, notes = update.advance("b", repo.head, root, push=False, check=mock.Mock(), docs_check=check)
            listed = repo.git("worktree", "list")
            return fresh, repo.head, notes, check, listed

    def test_two_that_each_fit_and_overflow_merged_are_handed_back_unpushed(self):
        fresh, _, notes, check, listed = self.take(mine=60, theirs=60)
        self.assertIsNone(fresh, "handed back")
        self.assertIn("[claude-md] 320/300", notes[0])
        check.assert_called_once()
        self.assertEqual(len(listed.splitlines()), 1, "the throwaway worktree is removed")

    def test_two_that_fit_merged_keep_the_run_on_record(self):
        fresh, head, notes, check, _ = self.take(mine=40, theirs=40)
        self.assertEqual(fresh, head)
        self.assertIn("only by Markdown", notes[0])
        check.assert_called_once()


class Which(unittest.TestCase):
    def test_only_a_head_touching_an_agent_doc_is_linted_on_a_docs_advance(self):
        def pull(*paths):
            return {"files": [{"path": p} for p in paths]}

        self.assertTrue(checks.touches_agent_doc(pull("docs/x.md", ".claude/skills/a/SKILL.md")))
        self.assertTrue(checks.touches_agent_doc(pull("src/a.rs", "src/render/CLAUDE.md")))
        self.assertFalse(checks.touches_agent_doc(pull("docs/x.md", "README.md", "src/a.rs")))


if __name__ == "__main__":
    unittest.main()
