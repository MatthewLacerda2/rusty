#!/usr/bin/env python3
"""#2215: under `merging/`, only `merging/state.py` writes a file.

A state file written in place can be cut short by a power cut and read back as
nothing (`merging.state`'s docstring). So every file write under `merging/`
outside `state.py` fails here: `write_text`, `write_bytes`, `json.dump`, and an
`open`/`fdopen` whose mode writes (`w`, `a`, `x`, `+`). A write that is not a
state file (a log append, a lock, a worktree file the resolver rewrites) says
so on its line: `# state-ok: <why>`, the why not empty.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

MERGING = Path(__file__).resolve().parents[2] / "merging"
WRITER = "state.py"

WRITES = re.compile(
    r"\.write_text\(|\.write_bytes\(|\bjson\.dump\("
    r"|\b(?:open|fdopen)\(.*?['\"][rwaxbt]*[wax+][rwaxbt+]*['\"]"
)
EXCEPTION = re.compile(r"#\s*state-ok:\s*\S")


def offences(text: str) -> list[str]:
    """The lines of one source file that write a file and do not say why."""
    return [line.strip() for line in text.splitlines() if WRITES.search(line) and not EXCEPTION.search(line)]


class Writers(unittest.TestCase):
    def test_nothing_under_merging_writes_a_file_but_state_py(self):
        found = {
            f"{path.relative_to(MERGING)}: {line}"
            for path in sorted(MERGING.rglob("*.py")) if path.relative_to(MERGING) != Path(WRITER)
            for line in offences(path.read_text())
        }
        self.assertEqual(found, set(), "write a state file through `merging.state.write`; a write that is "
                         "not one (an append, a lock) takes `# state-ok: <why>` on its line")

    def test_it_flags_each_way_of_writing(self):
        for line in ('path.write_text("{}")', "p.write_bytes(b'')", "json.dump(data, out)", 'open(p, "w")',
                     "with path.open('a') as out:", 'os.fdopen(fd, "r+")', 'open(p, mode="wb")',
                     'open(p, "w")  # state-ok:'):
            self.assertEqual(len(offences(line)), 1, line)

    def test_it_passes_reads_and_a_write_that_says_why(self):
        for line in ('open(p)', 'open(p, encoding="utf-8")', 'path.read_text()', 'json.dumps(x)',
                     'open(p, errors="replace")', 'open(log, "a") as out:  # state-ok: the log, appended'):
            self.assertEqual(offences(line), [], line)


if __name__ == "__main__":
    unittest.main()
