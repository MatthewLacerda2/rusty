"""`changes.agent_doc` mirrors `tools/lint`'s `agent_docs::is_agent_doc`: the same
cases as its Rust test, so the queue lints exactly the Markdown the gate holds."""

from __future__ import annotations

import unittest

from merging.verdict import changes


class AgentDoc(unittest.TestCase):
    def test_claude_md_files_and_skill_folders_files_are_agent_docs(self):
        for path in ("CLAUDE.md", "src/render/CLAUDE.md", ".claude/skills/a/SKILL.md", ".claude/skills/a/x/y.md"):
            self.assertTrue(changes.agent_doc(path), path)

    def test_other_markdown_is_not(self):
        for path in ("README.md", "docs/CLAUDE.md.bak", "XCLAUDE.md", ".claude/skills/loose.md", ".claude/x.md"):
            self.assertFalse(changes.agent_doc(path), path)


if __name__ == "__main__":
    unittest.main()
