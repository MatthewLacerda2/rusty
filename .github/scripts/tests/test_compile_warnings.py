#!/usr/bin/env python3
"""Every script compiles without a warning.

An invalid escape in a plain string (`"\\cargo"`) is a `SyntaxWarning` printed
on every run (a `DeprecationWarning` before Python 3.12) and an error in a later
Python. Importing a module does not show it once its `.pyc` is cached, so this
compiles every `.py` under `.github/scripts` and `tools/launcher`, tests
included, from source with warnings as errors. A string that means a backslash
is raw (`r"…"`) or doubles it.
"""

from __future__ import annotations

import unittest
import warnings

from tests import SCRIPTS

LAUNCHER = SCRIPTS.parent.parent / "tools" / "launcher"


def warning(source: str, name: str) -> str | None:
    """What compiling `source` warns or fails with, or None when it is clean."""
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        try:
            compile(source, name, "exec")
        except (SyntaxError, Warning) as e:
            return f"{name}: {e}"
    return None


class CompileWarnings(unittest.TestCase):
    def test_every_script_compiles_clean(self):
        found = [
            w
            for path in sorted([*SCRIPTS.rglob("*.py"), *LAUNCHER.rglob("*.py")])
            if (w := warning(path.read_text(encoding="utf-8"), str(path.relative_to(SCRIPTS.parent.parent))))
        ]
        self.assertEqual(found, [], "make the string raw (r\"…\") or double the backslash")

    def test_an_invalid_escape_is_flagged(self):
        self.assertIsNotNone(warning('"""a `\\cargo` command"""\n', "x.py"))

    def test_a_raw_or_doubled_backslash_passes(self):
        self.assertIsNone(warning('r"""a `\\cargo` command"""\n', "x.py"))
        self.assertIsNone(warning('"""a `\\\\cargo` command"""\n', "x.py"))


if __name__ == "__main__":
    unittest.main()
