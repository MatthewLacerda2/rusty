#!/usr/bin/env python3
"""The GPU suite's settings stay where docs/testing.md puts them (#978).

Two rules written in several files and held equal by hand until now:

- **The cap.** nextest's `gpu` test group and `MAX_CONCURRENT_HEADLESS`
  (`src/render/setup/budget.rs`) bound the same thing for the two runners; one
  edited alone is the #366 out-of-memory red. Beside it, the nextest settings
  the doc promises: no retries, `fail-fast = false`, the 60 s / 5-period kill.
- **An adapter is required wherever the suite runs on CI.** A job that runs the
  tests without `RUSTY_REQUIRE_GPU=1` skips every GPU test when its driver goes
  missing, and reads green.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

try:
    import tomllib  # 3.11+; CI's runner has it, an older local python3 skips
except ModuleNotFoundError:
    tomllib = None

REPO = Path(__file__).resolve().parents[3]
WORKFLOWS = REPO / ".github" / "workflows"
NEXTEST = tomllib.loads((REPO / ".config" / "nextest.toml").read_text()) if tomllib else {}
BUDGET = REPO / "src" / "render" / "setup" / "budget.rs"

#: A command that runs the engine's tests. `cargo test --doc` renders nothing,
#: and `tools/lint` is its own crate with no renderer.
SUITE = re.compile(r"cargo (nextest run|llvm-cov|mutants|test)\b")
NOT_SUITE = re.compile(r"--doc\b|tools/lint/")


def headless_cap() -> int:
    found = re.search(r"const MAX_CONCURRENT_HEADLESS: usize = (\d+);", BUDGET.read_text())
    assert found, f"MAX_CONCURRENT_HEADLESS not found in {BUDGET}"
    return int(found[1])


def jobs(text: str) -> dict[str, str]:
    """Each top-level job's name and its body, comments dropped."""
    text = "\n".join(l for l in text.splitlines() if not l.lstrip().startswith("#"))
    parts = re.split(r"^  ([\w-]+):\n", text.split("\njobs:\n", 1)[1], flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


def runs_suite(body: str) -> bool:
    return any(SUITE.search(l) and not NOT_SUITE.search(l) for l in body.splitlines())


def unguarded(text: str) -> list[str]:
    """The jobs that run the suite with no `RUSTY_REQUIRE_GPU: "1"` (job or workflow env)."""
    if re.search(r'(?m)^  RUSTY_REQUIRE_GPU: "1"$', text.split("\njobs:\n", 1)[0]):
        return []
    return [name for name, body in jobs(text).items()
            if runs_suite(body) and 'RUSTY_REQUIRE_GPU: "1"' not in body]


NEEDS_TOML = unittest.skipUnless(tomllib, "tomllib needs Python 3.11+")


@NEEDS_TOML
class Cap(unittest.TestCase):
    def test_the_two_caps_are_equal(self):
        self.assertEqual(NEXTEST["test-groups"]["gpu"]["max-threads"], headless_cap())


@NEEDS_TOML
class NextestProfile(unittest.TestCase):
    def test_default_profile_keeps_its_promises(self):
        default = NEXTEST["profile"]["default"]
        self.assertEqual(default["retries"], 0)
        self.assertIs(default["fail-fast"], False)
        self.assertEqual(default["slow-timeout"], {"period": "60s", "terminate-after": 5})

    def test_no_profile_overrides_them(self):
        for name, profile in NEXTEST["profile"].items():
            if name == "default":
                continue
            with self.subTest(profile=name):
                self.assertFalse({"retries", "fail-fast", "slow-timeout"} & profile.keys())

    def test_nothing_passes_retries_on_the_command_line(self):
        for path in [REPO / "Makefile", *sorted(WORKFLOWS.glob("*.yml"))]:
            with self.subTest(file=path.name):
                self.assertNotRegex(path.read_text(), r"--retries\b|NEXTEST_RETRIES")


class RequireGpu(unittest.TestCase):
    def test_every_workflow_job_that_runs_the_suite_requires_an_adapter(self):
        for path in sorted(WORKFLOWS.glob("*.yml")):
            with self.subTest(workflow=path.name):
                self.assertEqual(unguarded(path.read_text()), [])

    def test_flags_a_suite_job_without_it(self):
        bare = "on: push\njobs:\n  cov:\n    steps:\n      - run: cargo llvm-cov nextest --no-report\n"
        self.assertEqual(unguarded(bare), ["cov"])
        self.assertEqual(unguarded(bare.replace("llvm-cov nextest", "test --locked")), ["cov"])

    def test_passes_a_guarded_job_and_jobs_that_render_nothing(self):
        guarded = ('on: push\njobs:\n  t:\n    env:\n      RUSTY_REQUIRE_GPU: "1"\n'
                   "    steps:\n      - run: cargo nextest run --profile ci\n")
        self.assertEqual(unguarded(guarded), [])
        docs = ("on: push\njobs:\n  d:\n    steps:\n      - run: cargo test --doc --locked\n"
                "      - run: cargo test --manifest-path tools/lint/Cargo.toml\n"
                "      # cargo mutants, only in a comment\n")
        self.assertEqual(unguarded(docs), [])
        workflow_env = 'env:\n  RUSTY_REQUIRE_GPU: "1"\n' + guarded.replace("RUSTY_REQUIRE_GPU", "OTHER")
        self.assertEqual(unguarded(workflow_env), [])


if __name__ == "__main__":
    unittest.main()
