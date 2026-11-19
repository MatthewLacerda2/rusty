#!/usr/bin/env python3
"""The GPU suite's settings stay where docs/testing.md puts them (#978).

Two rules written in several files and held equal by hand until now:

- **The cap, per platform** (#2363). nextest's GPU test group for a platform
  (`gpu`, or `gpu-macos` on macOS, picked by a platform-keyed override) and that
  platform's `MAX_CONCURRENT_HEADLESS` (`crates/render/src/setup/budget.rs`, one
  per `cfg(target_os)`) bound the same thing for the two runners; one edited alone
  is the #366 out-of-memory red. Every GPU override keeps the one filter. Beside
  it, the nextest settings the doc promises: no retries, `fail-fast = false`, the
  60 s / 5-period kill.
- **An adapter is required wherever the suite runs on CI.** A job that runs the
  tests without `RUSTY_REQUIRE_GPU=1` skips every GPU test when its driver goes
  missing, and reads green.
- **A mutation run can't outgrow its runner.** `ulimit -v` caps each process
  (#674), and nextest runs `--test-threads` test processes at once, so the cap
  times the threads stays within the runner's memory (#1844).
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
BUDGET = REPO / "crates" / "render" / "src" / "setup" / "budget.rs"

#: A command that runs the engine's tests. `cargo test --doc` renders nothing,
#: `tools/lint` is its own crate with no renderer, and `cargo mutants --list`
#: only counts mutants (`mutants-on-request.yml`'s `plan` job).
SUITE = re.compile(r"cargo (nextest run|llvm-cov|mutants|test)\b")
NOT_SUITE = re.compile(r"--doc\b|tools/lint/|cargo mutants --list\b")


#: The platforms a cap is written for: a `cfg(target_os = …)` predicate's name, or
#: `None` for every other platform (no `platform` key, `cfg(not(…))`).
MACOS = 'cfg(target_os = "macos")'


def headless_caps(source: str) -> dict[str | None, int]:
    """`MAX_CONCURRENT_HEADLESS` per platform: `MACOS` under its `cfg`, `None`
    under `cfg(not(…))` or none."""
    found = re.findall(r'(?:#\[cfg\((not\()?target_os = "macos"\)?\)\]\n)?'
                       r"pub\(crate\) const MAX_CONCURRENT_HEADLESS: usize = (\d+);", source)
    assert found, "MAX_CONCURRENT_HEADLESS not found in budget.rs"
    caps: dict[str | None, int] = {}
    for negated, value in found:
        macos_only = not negated and len(found) > 1
        caps[MACOS if macos_only else None] = int(value)
    return caps


def nextest_caps(config: dict) -> dict[str | None, int]:
    """Each platform's GPU group cap: the group a GPU override (one whose group's
    name starts `gpu`) puts its tests in, by the override's `platform`."""
    groups = config["test-groups"]
    return {o.get("platform"): groups[o["test-group"]]["max-threads"]
            for o in config["profile"]["default"]["overrides"]
            if o.get("test-group", "").startswith("gpu")}


def gpu_filters(config: dict) -> set[str]:
    return {o["filter"] for o in config["profile"]["default"]["overrides"]
            if o.get("test-group", "").startswith("gpu")}


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


#: `ubuntu-latest`'s memory in KiB, what `ulimit -v` takes: 16 GB.
RUNNER_KIB = 16 * 1024 * 1024
MUTATE = re.compile(r"cargo mutants (?!--list\b)")


def overcommitted(text: str) -> list[str]:
    """The jobs running mutants whose memory cap times test threads exceeds the
    runner's, or that leave either unset (nextest's default is the CPU count)."""
    bad = []
    for name, body in jobs(text).items():
        if not MUTATE.search(body):
            continue
        cap = re.search(r"ulimit -v (\d+)", body)
        threads = re.search(r"--test-threads[ =](\d+)", body)
        if not (cap and threads) or int(cap[1]) * int(threads[1]) > RUNNER_KIB:
            bad.append(name)
    return bad


NEEDS_TOML = unittest.skipUnless(tomllib, "tomllib needs Python 3.11+")


@NEEDS_TOML
class Cap(unittest.TestCase):
    def test_each_platforms_two_caps_are_equal(self):
        self.assertEqual(nextest_caps(NEXTEST), headless_caps(BUDGET.read_text()))

    def test_every_gpu_override_keeps_the_one_filter(self):
        self.assertEqual(len(gpu_filters(NEXTEST)), 1)

    def test_flags_a_platform_whose_caps_differ(self):
        config = tomllib.loads(
            '[test-groups]\ngpu = { max-threads = 2 }\ngpu-macos = { max-threads = 3 }\n'
            f"[[profile.default.overrides]]\nplatform = '{MACOS}'\nfilter = 'a'\n"
            'test-group = "gpu-macos"\n[[profile.default.overrides]]\nfilter = \'a\'\ntest-group = "gpu"\n')
        budget = (f'#[cfg(target_os = "macos")]\npub(crate) const MAX_CONCURRENT_HEADLESS: usize = {{}};\n'
                  '#[cfg(not(target_os = "macos"))]\npub(crate) const MAX_CONCURRENT_HEADLESS: usize = 2;\n')
        self.assertEqual(nextest_caps(config), headless_caps(budget.format(3)))
        self.assertNotEqual(nextest_caps(config), headless_caps(budget.format(2)))

    def test_one_cap_for_every_platform_reads_as_one(self):
        config = tomllib.loads('[test-groups]\ngpu = { max-threads = 2 }\n'
                               '[[profile.default.overrides]]\nfilter = \'a\'\ntest-group = "gpu"\n')
        self.assertEqual(nextest_caps(config), {None: 2})
        self.assertEqual(headless_caps("pub(crate) const MAX_CONCURRENT_HEADLESS: usize = 2;\n"), {None: 2})
        self.assertNotEqual(nextest_caps(config), headless_caps(
            '#[cfg(target_os = "macos")]\npub(crate) const MAX_CONCURRENT_HEADLESS: usize = 3;\n'
            '#[cfg(not(target_os = "macos"))]\npub(crate) const MAX_CONCURRENT_HEADLESS: usize = 2;\n'))


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
        self.assertEqual(unguarded(bare.replace("llvm-cov nextest --no-report", "mutants --no-shuffle")), ["cov"])

    def test_passes_a_guarded_job_and_jobs_that_render_nothing(self):
        guarded = ('on: push\njobs:\n  t:\n    env:\n      RUSTY_REQUIRE_GPU: "1"\n'
                   "    steps:\n      - run: cargo nextest run --profile ci\n")
        self.assertEqual(unguarded(guarded), [])
        docs = ("on: push\njobs:\n  d:\n    steps:\n      - run: cargo test --doc --locked\n"
                "      - run: cargo test --manifest-path tools/lint/Cargo.toml\n"
                "      - run: cargo mutants --list --file 'src/app/**/*.rs' > list.txt\n"
                "      # cargo mutants, only in a comment\n")
        self.assertEqual(unguarded(docs), [])
        workflow_env = 'env:\n  RUSTY_REQUIRE_GPU: "1"\n' + guarded.replace("RUSTY_REQUIRE_GPU", "OTHER")
        self.assertEqual(unguarded(workflow_env), [])


class MutationMemory(unittest.TestCase):
    def test_every_mutation_job_fits_its_runner(self):
        for path in sorted(WORKFLOWS.glob("mutants-*.yml")):
            with self.subTest(workflow=path.name):
                self.assertEqual(overcommitted(path.read_text()), [])

    def test_flags_unbounded_threads_or_too_big_a_product(self):
        job = ("on: push\njobs:\n  m:\n    steps:\n      - run: |\n          ulimit -v 8388608\n"
               "          cargo mutants --test-tool nextest -- --features dev{}\n")
        self.assertEqual(overcommitted(job.format("")), ["m"])
        self.assertEqual(overcommitted(job.format(" --test-threads 4")), ["m"])
        self.assertEqual(overcommitted(job.format(" --test-threads 2").replace("ulimit -v 8388608\n", "")), ["m"])

    def test_passes_a_bounded_job_and_a_mutant_count(self):
        job = ("on: push\njobs:\n  m:\n    steps:\n      - run: |\n          ulimit -v 8388608\n"
               "          cargo mutants --test-tool nextest -- --test-threads 2\n"
               "  plan:\n    steps:\n      - run: cargo mutants --list --json > p.json\n")
        self.assertEqual(overcommitted(job), [])


if __name__ == "__main__":
    unittest.main()
