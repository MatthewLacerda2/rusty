"""`changes.agent_doc` mirrors `tools/lint`'s `agent_docs::is_agent_doc`: the same
cases as its Rust test, so the queue lints exactly the Markdown the gate holds.
`changes.drift_ran` reads `build-test`'s matrix (#2264)."""

from __future__ import annotations

import unittest

from merging.verdict import changes


class AgentDoc(unittest.TestCase):
    def test_claude_md_files_and_skill_folders_files_are_agent_docs(self):
        for path in ("CLAUDE.md", "src/render/CLAUDE.md", ".claude/skills/a/SKILL.md", ".claude/skills/a/x/y.md",
                     ".claude/agents/coder.md"):
            self.assertTrue(changes.agent_doc(path), path)

    def test_other_markdown_is_not(self):
        for path in ("README.md", "docs/CLAUDE.md.bak", "XCLAUDE.md", ".claude/skills/loose.md", ".claude/x.md",
                     ".claude/agents/x/y.md", ".claude/agents/y.txt"):
            self.assertFalse(changes.agent_doc(path), path)


DEV = "Test (engine, dev features)"


def shard(name: str, conclusion: str) -> dict:
    return {"name": name, "steps": [{"name": DEV, "conclusion": conclusion}]}


class DriftRan(unittest.TestCase):
    def ran(self, *jobs: dict) -> bool:
        return changes.drift_ran({"id": 1}, {1: list(jobs)})

    def test_every_dev_shard_passing_counts(self):
        self.assertTrue(self.ran(shard("build-test (default, 1/1)", "skipped"), shard("build-test (dev, 1/2)", "success"),
                                 shard("build-test (dev, 2/2)", "success")))

    def test_a_shard_failed_or_cancelled_or_none_ran_does_not(self):
        for other in ("failure", "cancelled"):
            with self.subTest(other=other):
                self.assertFalse(self.ran(shard("build-test (dev, 1/2)", "success"), shard("build-test (dev, 2/2)", other)))
        self.assertFalse(self.ran(shard("build-test (dev, 1/2)", "skipped")))
        self.assertFalse(self.ran(shard("build-test-cross (macos-26)", "success")))


if __name__ == "__main__":
    unittest.main()
