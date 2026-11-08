#!/usr/bin/env python3
"""#1956: two heads under the cap whose merge is over it are handed back before any push.

Real repositories (a bare `origin` and a clone, as `test_update_by_merge`), and a
stand-in `tools/lint`: [`StandIn`] answers the build by writing a shell script
where cargo would put the binary, and that script fails the size pass when a
file is over [`CAP`] lines, writing `.lint/report.txt` as the real one does. So
the build, the copy kept per `tools/lint` tree, the run in the merged tree and
the report are all real; only rustc is not asked.
"""

from __future__ import annotations

import os
import stat
import subprocess
import tempfile
import unittest
from unittest import mock

from merging.queue import lints, update
from tests.queue.test_markdown_advance import WHO
from tests.queue.test_update_by_merge import Origin, run


CAP = 9

# The size pass of the stand-in: every other pass (a flag) passes.
SCRIPT = f"""#!/bin/sh
[ $# -gt 0 ] && exit 0
mkdir -p .lint
bad=$(git ls-files '*.rs' | while read f; do n=$(wc -l < "$f"); [ "$n" -gt {CAP} ] && echo "FILE_TOO_LONG $f $n/{CAP}"; done)
if [ -n "$bad" ]; then printf 'lint: FAILED\\n%s\\n' "$bad" > .lint/report.txt; exit 1; fi
echo 'lint: ok' > .lint/report.txt
"""


class StandIn:
    """`subprocess.run` for a [`lints.Linter`]: cargo builds [`SCRIPT`], the rest runs."""

    def __init__(self):
        self.builds = 0

    def __call__(self, argv, **kwargs):
        if argv[0] != "cargo":
            return subprocess.run(argv, **kwargs)
        self.builds += 1
        exe = os.path.join(kwargs["env"]["CARGO_TARGET_DIR"], "release", lints.binary_name())
        os.makedirs(os.path.dirname(exe), exist_ok=True)
        with open(exe, "w") as out:
            out.write(SCRIPT)
        os.chmod(exe, os.stat(exe).st_mode | stat.S_IEXEC)
        return subprocess.CompletedProcess(argv, 0, "", "")


def lines(n: int, tag: str) -> str:
    return "".join(f"// {tag} {i}\n" for i in range(n))


class MergedOverTheCap(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        env = mock.patch.dict(os.environ, WHO)
        env.start()
        self.addCleanup(env.stop)
        self.repo = Origin(self.tmp.name)
        # A shared file of 5 lines on the base, and a `tools/lint` for the tree hash.
        self.repo.commit({"src/play.rs": lines(5, "base"), "tools/lint/Cargo.toml": "[package]\n"}, "main")
        self.repo.git("checkout", "--quiet", "b")
        self.repo.git("merge", "--quiet", "--no-edit", "main")
        self.repo.git("push", "--quiet", "origin", "HEAD:refs/heads/b")
        self.said: list[str] = []
        self.runner = StandIn()
        self.linter = lints.Linter(os.path.join(self.tmp.name, "lint-target"), runner=self.runner, say=self.said.append)

    def grow(self, branch: str, top: int, bottom: int, subject: str) -> str:
        """Add `top` lines before and `bottom` after `src/play.rs`'s base lines on `branch`."""
        self.repo.git("checkout", "--quiet", branch)
        path = os.path.join(self.repo.root, "src", "play.rs")
        with open(path, "w") as out:
            out.write(lines(top, f"{branch} top") + lines(5, "base") + lines(bottom, f"{branch} bottom"))
        self.repo.git("commit", "--quiet", "-am", subject)
        self.repo.git("push", "--quiet", "origin", f"HEAD:refs/heads/{branch}")
        self.repo.git("fetch", "--quiet", "origin")
        return self.repo.git("rev-parse", "HEAD").strip()

    def advance(self, head: str):
        check = lambda work: self.linter.check(work, since=head, upto="origin/main")  # noqa: E731
        return update.advance("b", head, self.repo.root, push=True, check=check)

    def test_each_side_under_the_cap_merged_over_it_is_handed_back_unpushed(self):
        self.grow("main", top=3, bottom=0, subject="Grow play (#1942) (#12)")  # 8 lines on main
        head = self.grow("b", top=0, bottom=3, subject="on b")  # 8 lines on the branch
        fresh, notes = self.advance(head)
        self.assertIsNone(fresh)
        self.assertEqual(notes[0], "the merged tree fails tools/lint; not pushed.")
        self.assertIn("FILE_TOO_LONG src/play.rs 11/9", notes)
        self.assertIn("src/play.rs was also changed by #12, merged since this branch's base.", notes)
        self.assertEqual(notes[-1], lints.HANDED_BACK)
        self.assertEqual(self.repo.on_origin("b"), head)  # nothing pushed

    def test_a_clean_merge_under_the_cap_is_pushed_and_the_build_is_reused(self):
        self.grow("main", top=1, bottom=0, subject="a line (#12)")
        head = self.grow("b", top=0, bottom=1, subject="on b")
        fresh, notes = self.advance(head)
        self.assertIsNotNone(fresh)
        self.assertEqual(self.repo.on_origin("b"), fresh)
        self.assertEqual(notes, [])
        # A second take with the same `tools/lint` runs the kept binary: no build.
        self.grow("main", top=2, bottom=0, subject="another (#13)")
        self.repo.git("fetch", "--quiet", "origin")
        fresh, _ = self.advance(fresh)
        self.assertIsNotNone(fresh)
        self.assertEqual(self.runner.builds, 1)
        self.assertEqual(self.said, [])

    def test_a_branch_changing_tools_lint_gets_a_build_of_its_own(self):
        self.grow("main", top=1, bottom=0, subject="a line (#12)")
        fresh, _ = self.advance(run("git", "rev-parse", "origin/b", cwd=self.repo.root).strip())
        self.repo.git("checkout", "--quiet", "-B", "b", fresh)
        self.repo.commit({"tools/lint/src.rs": "fn main() {}\n"}, "b")
        self.grow("main", top=2, bottom=0, subject="another (#13)")
        head = run("git", "rev-parse", "origin/b", cwd=self.repo.root).strip()
        self.assertIsNotNone(self.advance(head)[0])
        self.assertEqual((self.runner.builds, len(self.linter.built)), (2, 2))

if __name__ == "__main__":
    unittest.main()
