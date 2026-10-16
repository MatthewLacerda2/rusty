#!/usr/bin/env python3
"""No script calls a GraphQL `gh` subcommand (#970).

`gh pr …`, `gh issue …` and `gh repo …` speak GraphQL, which a cloud session's
proxy refuses with a 403, so `make blockers` died there at once. The scripts go
through `gh api` (REST) and `ghrest`. This scans every script under
`.github/scripts` (its tests aside) for such a call: an argv literal like
`"pr", "view"` in Python, or a `gh pr …` command line in shell. Prose that names
one is not a call and passes. An exception goes on the line itself, as
`# graphql-ok: <reason>`.
"""

from __future__ import annotations

import re
import unittest

from tests import SCRIPTS

SUBCOMMANDS = r"(?:pr|issue|repo)"
PYTHON_CALL = re.compile(rf'"{SUBCOMMANDS}",\s*"[a-z-]+"')
SHELL_CALL = re.compile(rf"^\s*[^#]*\bgh\s+{SUBCOMMANDS}\s+[a-z-]+")
EXCEPTION = "# graphql-ok:"


def offences(path: str, text: str) -> list[str]:
    """`path:line: text` for every GraphQL `gh` call in `text`."""
    call = PYTHON_CALL if path.endswith(".py") else SHELL_CALL
    return [
        f"{path}:{n}: {line.strip()}"
        for n, line in enumerate(text.splitlines(), 1)
        if call.search(line) and EXCEPTION not in line
    ]


class RestOnly(unittest.TestCase):
    def test_no_script_calls_graphql(self):
        found = []
        for path in sorted([*SCRIPTS.rglob("*.py"), *SCRIPTS.rglob("*.sh")]):
            if "tests" in path.relative_to(SCRIPTS).parts:
                continue
            found += offences(str(path.relative_to(SCRIPTS)), path.read_text())
        self.assertEqual(found, [], "use `gh api repos/{o}/{r}/…` and ghrest, not GraphQL (#970)")

    def test_a_graphql_call_is_flagged(self):
        self.assertTrue(offences("x.py", 'gh("pr", "view", "1", "--json", "headRefOid")'))
        self.assertTrue(offences("x.py", 'run(["gh", "issue", "list"])'))
        self.assertTrue(offences("x.sh", 'n=$(gh pr view 1 --json number)'))

    def test_rest_prose_and_a_written_exception_pass(self):
        self.assertEqual(offences("x.py", 'gh("api", f"repos/{repo}/pulls/1")'), [])
        self.assertEqual(offences("x.py", '"""What `gh pr list` used to return."""'), [])
        self.assertEqual(offences("x.sh", "# gh pr view is GraphQL"), [])
        self.assertEqual(offences("x.sh", "gh pr view 1  # graphql-ok: runs only in CI"), [])
        self.assertEqual(offences("x.sh", 'gh api "repos/$REPO/pulls/1"'), [])


if __name__ == "__main__":
    unittest.main()
