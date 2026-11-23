"""#2628: after a merge, the main checkout moves forward only, and only on `main`.

Must move: a checkout on `main` that fast-forwards. Must not: one on another
branch (said, left as it is), and a fetch or a fast-forward that fails is said.
"""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

from merging.view import chores


class Git:
    def __init__(self, branch: str = "main", failing: str | None = None):
        self.branch, self.failing, self.asked = branch, failing, []

    def __call__(self, *args: str, cwd: str | None = None) -> subprocess.CompletedProcess:
        self.asked.append(args[0])
        out = self.branch + "\n" if args[0] == "symbolic-ref" else ""
        code = 1 if args[0] == self.failing else 0
        return subprocess.CompletedProcess(args, code, out, "fatal: Not possible to fast-forward, aborting." if code else "")


class Forward(unittest.TestCase):
    def test_a_checkout_on_main_fast_forwards(self):
        git = Git()
        self.assertEqual(chores.forward(Path("/m"), 5, git), [])
        self.assertEqual(git.asked, ["symbolic-ref", "fetch", "merge"])

    def test_a_checkout_elsewhere_is_left_alone(self):
        git = Git(branch="2628-x")
        said = chores.forward(Path("/m"), 5, git)
        self.assertIn("not on `main`", said[0])
        self.assertEqual(git.asked, ["symbolic-ref"])

    def test_a_failure_is_said(self):
        for step in ("fetch", "merge"):
            with self.subTest(step=step):
                said = chores.forward(Path("/m"), 5, Git(failing=step))
                self.assertEqual(len(said), 1)
                self.assertIn("#5 merged", said[0])

    def test_the_main_checkout_is_the_common_dirs_parent(self):
        self.assertEqual(chores.main_checkout(Path("/repo/.git")), Path("/repo"))


if __name__ == "__main__":
    unittest.main()
