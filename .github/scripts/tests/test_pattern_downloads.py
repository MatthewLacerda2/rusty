#!/usr/bin/env python3
"""Every artifact is downloaded by name, or merged on purpose (#2284, #2288).

`actions/download-artifact` with `pattern:` and `path: <dir>` unpacks each match
into `<dir>/<artifact-name>/`, except when the pattern matches exactly one
artifact: that one is unpacked flat into `<dir>/` itself. A step that reads
`<dir>/<name>-*/` then finds nothing, and the lone artifact reads as absent.
It bit twice, both times a lone mutation shard reported as lost. So a
`pattern:` download fails here unless it sets `merge-multiple: true` with a `#`
reason on that line (one directory for every match, on purpose); otherwise
download each artifact by `name:`, one step each, as `mutants-on-request.yml`'s
`report` job does with its shards.
"""

from __future__ import annotations

import re
import unittest

from tests import SCRIPTS

WORKFLOW_DIR = SCRIPTS.parent / "workflows"
ACTIONS_DIR = SCRIPTS.parent / "actions"


def jobs_steps(workflow: str) -> list[list[str]]:
    """Each `steps:` list in a workflow's text, every step as its own text.

    Indentation alone, no YAML parser (the scripts are stdlib only): a step is an
    item `- ` at the list's first item's indent, until the next one or a line at
    or left of `steps:` itself. Comment lines are dropped, and so is a trailing
    comment on the `steps:` line (`steps:  # why`), which still opens a list.
    """
    blocks, lines = [], workflow.splitlines()
    for i, line in enumerate(lines):
        if re.sub(r"\s#.*$", "", line).strip() != "steps:":
            continue
        depth, item, block = len(line) - len(line.lstrip()), None, []
        for body in lines[i + 1:]:
            indent, text = len(body) - len(body.lstrip()), body.strip()
            if not text or text.startswith("#"):
                continue
            if indent <= depth and not text.startswith("- "):
                break
            if text.startswith("- ") and item in (None, indent):
                item = indent
                block.append([body])
            elif block:
                block[-1].append(body)
        blocks.append(["\n".join(step) for step in block])
    return blocks


def pattern_downloads(workflow: str) -> list[str]:
    """Each `pattern:` download whose lone match would unpack flat, with what to do."""
    found = []
    for block in jobs_steps(workflow):
        for step in block:
            if "actions/download-artifact@" not in step or not re.search(r"(?m)^\s+pattern:", step):
                continue
            if re.search(r"(?m)^\s+merge-multiple:\s*true\s+#\s*\S", step):
                continue
            first = step.strip().splitlines()[0].strip()
            found.append(
                f"`{first}` downloads by `pattern:`, which unpacks a lone match flat: download each "
                f"artifact by `name:` instead, or set `merge-multiple: true  # <why one directory is right>`.")
    return found


DOWNLOAD = """jobs:
  report:
    steps:
      - name: Download
        uses: actions/download-artifact@v8
        with:
          {key}: shard-*
          path: shards{extra}
"""


class PatternDownloads(unittest.TestCase):
    def test_every_workflow_and_action_downloads_by_name(self):
        paths = sorted(WORKFLOW_DIR.glob("*.yml")) + sorted(ACTIONS_DIR.glob("*/action.yml"))
        self.assertTrue(any(p.name == "action.yml" for p in paths), "no composite action found")
        for path in paths:
            with self.subTest(path=str(path.relative_to(SCRIPTS.parent))):
                self.assertEqual(pattern_downloads(path.read_text()), [])

    def test_a_download_by_name_passes(self):
        self.assertEqual(pattern_downloads(DOWNLOAD.format(key="name", extra="")), [])

    def test_merge_multiple_with_a_reason_passes(self):
        extra = "\n          merge-multiple: true  # every shard's files are disjoint"
        self.assertEqual(pattern_downloads(DOWNLOAD.format(key="pattern", extra=extra)), [])

    def test_a_pattern_download_is_flagged(self):
        cases = {
            "bare": DOWNLOAD.format(key="pattern", extra=""),
            "merge-multiple without a reason": DOWNLOAD.format(key="pattern", extra="\n          merge-multiple: true"),
            "a comment on the `steps:` line": DOWNLOAD.replace("    steps:\n", "    steps:   # the report\n")
                                                     .format(key="pattern", extra=""),
            "in a composite action": "runs:\n  using: composite\n  steps:\n    - uses: actions/download-artifact@v8\n"
                                     "      with:\n        pattern: shard-*\n        path: shards\n",
        }
        for case, text in cases.items():
            with self.subTest(case=case):
                found = pattern_downloads(text)
                self.assertEqual(len(found), 1)
                self.assertIn("name:", found[0])


if __name__ == "__main__":
    unittest.main()
