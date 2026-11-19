#!/usr/bin/env python3
"""#2381: each binary is asked its `--help` before the texture check, so `main`'s
from before `--pending-imports` never reads the flag as its out folder."""

from __future__ import annotations

import importlib.util
import os
import stat
import tempfile
import unittest
import unittest.mock
from pathlib import Path

HERE = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("bench_ab", HERE / "bench-ab.py")
bench_ab = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench_ab)


def fake(dir: Path, usage: str, pending_exit: int = 0, listed: str = "") -> tuple[Path, Path]:
    """A `bench-scenes` stand-in: `--help` prints `usage`; every other call is logged."""
    log, binary = dir / "calls.log", dir / "bench-scenes"
    binary.write_text(f"""#!/bin/sh
if [ "$1" = --help ]; then echo '{usage}'; exit 0; fi
echo "$@" >> '{log}'
echo '[Project] Opened x' >&2
[ -n '{listed}' ] && echo '{listed}'
exit {pending_exit}
""")
    binary.chmod(binary.stat().st_mode | stat.S_IXUSR)
    return binary, log


OLD = "usage: bench-scenes --project <dir> [--before <dir>] <out_dir>"
NEW = OLD + " [--set <k=v>]... | --project <dir> --pending-imports"


class Ask(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())

    def test_an_old_main_cannot_be_asked_and_is_never_benched(self):
        binary, log = fake(self.dir, OLD)
        kind, said = bench_ab.ask_binary(binary, self.dir, (), "main")
        self.assertEqual(kind, "cannot")
        self.assertIn("could not run", said)
        self.assertFalse(log.exists(), "no run with --pending-imports read as its out folder")
        with self.assertRaises(SystemExit):
            bench_ab.check_textures((kind, said), ("clean", None), bench_ab.Gate(False))
        gate = bench_ab.Gate(True)
        bench_ab.check_textures((kind, said), ("clean", None), gate)
        self.assertIn("could not run", gate.overridden[0])

    def test_a_set_on_an_old_binary_refuses_even_under_force(self):
        binary, _ = fake(self.dir, OLD)
        with self.assertRaises(SystemExit) as e:
            bench_ab.ask_binary(binary, self.dir, ("a=1",), "main")
        self.assertIn("no --set", str(e.exception.code))

    def test_a_new_binary_is_asked_on_stdout_alone(self):
        binary, log = fake(self.dir, NEW)
        self.assertEqual(bench_ab.ask_binary(binary, self.dir, ("a=1",), "main"), ("clean", None))
        self.assertEqual(log.read_text().split(), ["--project", str(self.dir), "--pending-imports"])
        binary, _ = fake(Path(tempfile.mkdtemp()), NEW, pending_exit=1, listed="a.png")
        kind, said = bench_ab.ask_binary(binary, self.dir, (), "the PR")
        self.assertEqual(kind, "pending")
        self.assertIn("the PR's build", said)
        self.assertIn("a.png", said)
        self.assertNotIn("[Project] Opened", said, "stderr is not the list")

    def test_a_json_object_set_is_a_usage_error(self):
        with open(os.devnull, "w") as quiet, unittest.mock.patch("sys.stderr", quiet):
            with self.assertRaises(SystemExit) as e:
                bench_ab.args(["--pr", "1", "--project", ".", "--set", "ssao.high={radius:1.5}"])
        self.assertEqual(e.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
