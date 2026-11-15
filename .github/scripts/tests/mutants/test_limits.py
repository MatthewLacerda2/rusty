#!/usr/bin/env python3
"""A memory cap in a workflow comes with core dumps off (#2230).

`ulimit -v` makes a mutant that allocates without bound die on its own
allocation (#674), as an abort. On GitHub's runners an abort is handed to
`systemd-coredump` with no size limit, and the process stays alive, holding
its memory, while its 7 GB core is written out at about 200 MB/s: longer than
the mutant's test timeout, so cargo-mutants moves on with the dying process
still there, and a few such mutants in a row took the runner down (runs
43683626535 and 43688877549, the `externs` scope). With `ulimit -c 0` the
same mutant aborts in 5 s, frees its memory at once and reads as caught.
So every `ulimit` line that sets `-v` sets `-c 0` too.
"""

from __future__ import annotations

import re
import unittest

from tests import SCRIPTS

WORKFLOWS = SCRIPTS.parent / "workflows"
ULIMIT = re.compile(r"^\s*ulimit\b.*$", re.M)


def uncapped_dumps(text: str) -> list[str]:
    """The `ulimit` lines that cap memory and leave core dumps on."""
    return [line.strip() for line in ULIMIT.findall(text)
            if re.search(r"\s-v\s", line + " ") and not re.search(r"\s-c\s+0\b", line)]


class Workflows(unittest.TestCase):
    def test_every_memory_cap_turns_core_dumps_off(self) -> None:
        for path in sorted(WORKFLOWS.glob("*.yml")):
            with self.subTest(path.name):
                bad = uncapped_dumps(path.read_text())
                self.assertEqual(bad, [], f"{path.name}: add `-c 0` to {bad}: an abort at the memory cap "
                                          "otherwise writes a core as large as the cap (test_limits.py says why)")


class Cases(unittest.TestCase):
    def test_a_cap_alone_is_flagged(self) -> None:
        self.assertEqual(uncapped_dumps("        run: |\n          ulimit -v 8388608\n"), ["ulimit -v 8388608"])

    def test_a_cap_with_dumps_off_passes(self) -> None:
        self.assertEqual(uncapped_dumps("          ulimit -v 8388608 -c 0\n          ulimit -c 0 -v 1\n"), [])

    def test_a_line_with_no_cap_is_not_asked(self) -> None:
        self.assertEqual(uncapped_dumps("          ulimit -n 4096\n"), [])


if __name__ == "__main__":
    unittest.main()
