#!/usr/bin/env python3
"""#2450: `bench-ab --stress` end to end with every build, git call and run faked:
the PR's tree holds `main` merged in through its last stress run and is put back after."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from contextlib import ExitStack, redirect_stdout, redirect_stderr
from io import StringIO
from pathlib import Path
from unittest import mock

from tests import load

bench_ab = load("bench-ab.py", "bench_ab_stress")


class Fake:
    """The machine `bench-ab` sees: a log of what it did, in order."""

    def __init__(self):
        self.root, self.log, self.space = Path(tempfile.mkdtemp()), [], []

    def take(self, root, name, sha, pid, locked, exclusive=False):
        tree = root / ".claude" / "worktrees" / name
        locked.append(tree)
        return tree

    def run(self, argv, **_):  # a leg's run, a `ps` or a `--help`
        if argv[0] == "ps":
            return subprocess.CompletedProcess(argv, 0, "", "")
        self.log.append(("run", argv[0], Path(argv[-1])))
        out = Path(argv[-1])
        out.mkdir(parents=True, exist_ok=True)
        tree = "pr" if "pr-7-head" in argv[0] else "main"
        (out / "bench.json").write_text(json.dumps({"metrics": [["cpu_ms", {"avg": 2.0 if tree == "pr" else 4.0}]]}))
        return subprocess.CompletedProcess(argv, 0, "", "")

    def main(self, *argv) -> tuple[int, str]:
        patches = [
            ("trees.main_checkout", lambda: self.root),
            ("trees.fetch", lambda pr, again: ("m" * 40, "p" * 40)),
            ("trees.take", self.take),
            ("trees.merge_main", lambda tree, pr, sha: self.log.append(("merge", tree.name)) or "x" * 40),
            ("trees.build", lambda tree, bins: self.log.append(("build", tree.name, bins)) or
             tree / "target/release" / bins[0]),
            ("trees.run", lambda argv: self.log.append(("put back", argv[2], argv[-1])) or ""),
            ("trees.unlock", lambda root, tree, pid: None),
            ("space.refusal", lambda root, builds, what, run: self.space.append(builds)),
            ("ask_binary", lambda binary, project, sets, build: self.log.append(("ask", build)) or ("clean", None)),
            ("capped_scope", lambda: ([], "legs run plain")),
            ("subprocess.run", self.run),
        ]
        out, err = StringIO(), StringIO()
        with ExitStack() as stack:
            for name, fake in patches:
                owner, _, attr = name.rpartition(".")
                stack.enter_context(mock.patch.object(getattr(bench_ab, owner) if owner else bench_ab, attr, fake))
            stack.enter_context(redirect_stdout(out))
            stack.enter_context(redirect_stderr(err))
            code = bench_ab.main(["--pr", "7", *argv])
        return code, out.getvalue() + err.getvalue()


class Order(unittest.TestCase):
    def test_every_pr_stress_run_comes_after_the_merge_and_before_the_put_back(self):
        fake = Fake()
        project = Path(tempfile.mkdtemp())
        code, said = fake.main("--project", str(project), "--stress", "--runs", "2")
        self.assertEqual(code, 0, said)
        kinds = [entry[0] for entry in fake.log]
        merge, back = kinds.index("merge"), kinds.index("put back")
        self.assertEqual(fake.log[back][1:], (str(fake.root / ".claude/worktrees/pr-7-head"), "p" * 40))
        yard = [i for i, e in enumerate(fake.log) if e[0] == "run" and e[2].name.startswith("stress-")]
        pr_yard = [i for i in yard if fake.log[i][2].parent.name == "pr"]
        self.assertEqual(len(yard), 6, "three legs of two runs")
        self.assertEqual(len(pr_yard), 2)
        self.assertTrue(merge < min(pr_yard) and max(yard) < back, "the PR's yard runs on the merged tree")
        scenes = [i for i, e in enumerate(fake.log) if e[0] == "run" and not e[2].name.startswith("stress-")]
        self.assertLess(max(scenes), min(yard), "the yard after the scenes' legs")
        self.assertIn(("build", "pr-7-head", ("bench-scenes", "play")), fake.log, "one invocation, both binaries")
        self.assertIn("**stress yard**", said)
        self.assertIn("**stress yard**", (fake.root / "out/bench/ab/pr-7/table.md").read_text())
        self.assertEqual(fake.space[0][0][2], ("play",), "the disk check counts the second binary")

    def test_the_yard_alone_builds_play_only_and_asks_no_texture_question(self):
        fake = Fake()
        code, said = fake.main("--stress")
        self.assertEqual(code, 0, said)
        self.assertNotIn(("ask", "main"), fake.log)
        self.assertIn(("build", "bench-main", ("play",)), fake.log)
        self.assertTrue(all(e[2].name.startswith("stress-") for e in fake.log if e[0] == "run"), "no scene leg")
        self.assertIn("the stress yard alone", said)
        self.assertEqual(fake.space[0][0][2], ())

    def test_without_stress_the_run_is_as_before(self):
        fake = Fake()
        code, said = fake.main("--project", str(Path(tempfile.mkdtemp())))
        self.assertEqual(code, 0, said)
        self.assertIn(("build", "pr-7-head", ("bench-scenes",)), fake.log)
        self.assertEqual([e[2].name for e in fake.log if e[0] == "run"], ["main", "pr", "main-again"])
        self.assertNotIn("stress yard", said)

    def test_scene_flags_without_a_project_and_no_ask_at_all_are_usage_errors(self):
        for argv in ((), ("--stress", "--scenes", "a")):
            with redirect_stderr(StringIO()), self.assertRaises(SystemExit) as stop:
                bench_ab.args(["--pr", "7", *argv])
            self.assertEqual(stop.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
