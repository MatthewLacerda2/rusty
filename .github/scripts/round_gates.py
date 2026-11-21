#!/usr/bin/env python3
"""`make round-gates`: the gates of what a review round touched (#2452).

A `Round:` session (a re-brief on a pushed branch) used to pay the full `make
gates`, about 25 minutes from an empty `target/`, to ready a ten-line fix: the
gates were 61% of a round's time (`make coder-cost`). The operator chose to run
only the gates a round's files can break; the pull request's CI and the merge
queue's tested tree still run everything before anything merges, so a round that
readies red is handed back as any red PR is. What it runs must agree with `make
gates` in both directions: no gate skipped that the diff can break, and no red
that CI would not show.

What runs, from the files the branch changes against its merge base with
`origin/main` (committed, edited and new, as `fuzz_gate.changed_paths` reads them):

- **`quick-gates`, always.** Seconds, and the only gates a docs-only diff gets.
- **`clippy`** when any `.rs` of the workspace changed: it compiles every crate's
  targets, so a signature change that breaks a dependent's code or tests is caught
  here even though only the changed crates' tests run. A change under `tools/lint`
  alone runs only `tools/lint`'s clippy, the line of the `Makefile`'s recipe that
  names it.
- **The tests of each workspace crate that holds a changed file**, as `make test`
  runs them: the same four runs over the whole workspace, with nextest's
  `package(...)` filter narrowing which tests *run*, not what is built. A `-p
  <crate>` build is not the configuration `make test` builds: cargo unifies
  features only across the selected packages, so the root's `editor`/`dev` and
  every other member's dependency features (glam's, serde's, ...) go missing, and
  naming them on the command line reaches only direct dependencies (measured on
  #2452: 24 of 27 crates differ). The doctests run whole, for the same reason.
  The crate of a path is the deepest member directory `cargo metadata` lists
  above it; the root package holds `src/` and `tests/`, so the integration binary
  runs only when one of those changed.
- **`doc`** when any `.rs` of the workspace changed; **`fuzz-check`** when the
  diff matches `ci.yml`'s `fuzz` filter (what `make gates` asks too); **`deny`**
  when `deny.toml` changed.

A change it cannot place falls back to the full `make gates` and says so: the
`Makefile`, a workflow or action, `Cargo.lock`, a manifest (it moves features
and dependencies across the graph, and `deny` and `fuzz-check` read it), a build
script, the toolchain or cargo/nextest config, any file under a `fixtures/`
directory (tests of crates its path does not name read them), Markdown that code
reads (`ci.yml`'s `code` filter: `docs/api/` is read by tests in several
crates), any path outside a crate that no rule here names (`engine/`, `games/`:
read by many crates), and a `cargo metadata` that fails.

It ends with one line a coder pastes into the PR under the round's heading: what
ran, on which commit, what it left to CI, and what CI skips for this diff too
(its `code` and `fuzz` filters). `--plan` prints the commands, each with the
gates' proptest seed it needs, and that line, and runs nothing. The suite, the
seed and the `cargo-nextest` hint are read from the `Makefile` and `ci.yml`, so
there is one copy of each. Stdlib only.
"""

from __future__ import annotations

import json
import os
import re
import shlex
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

import fuzz_gate
import gate_mirror

ROOT = fuzz_gate.ROOT
CI = ".github/workflows/ci.yml"
ROOT_PACKAGE = "rusty"
# The root package's own directories; the rest of the root is classed below.
ROOT_DIRS = ("src/", "tests/")
# Unplaceable: (glob, why) — any match runs `make gates` in full.
FALLBACK = [
    ("Makefile", "the gates' own recipes"),
    (".github/workflows/**", "a workflow"),
    (".github/actions/**", "a CI action"),
    ("Cargo.lock", "the lock moves every crate's dependencies"),
    ("**Cargo.toml", "a manifest moves features and dependencies across the graph"),
    ("rust-toolchain.toml", "the toolchain"),
    (".cargo/**", "cargo's config"),
    (".config/**", "nextest's config"),
    ("fixtures/**", "fixtures are read by tests of crates the path does not name"),
    ("**/fixtures/**", "fixtures are read by tests of crates the path does not name"),
]
# Paths whose every gate is in `quick-gates`; `tools/lint`'s `.rs` adds its clippy.
QUICK_ONLY = [".github/scripts/**", "tools/**", ".claude/**", ".githooks/**", "rustfmt.toml", "fuzz/**"]
CLIPPY_ONLY = ["clippy.toml"]
LINT = "tools/lint/"
MAKE = ["make", "--no-print-directory"]


@dataclass
class Plan:
    """What a round runs. `fallback` set means `make gates` in full, for that reason."""

    fallback: str | None = None
    clippy: bool = False
    lint_clippy: bool = False
    doc: bool = False
    fuzz: bool = False
    deny: bool = False
    code: bool = False  # CI's `code` filter matches: CI builds, tests and denies this diff
    crates: list[str] = field(default_factory=list)


@dataclass
class Recipes:
    """What the round runs, read from the `Makefile` and `ci.yml`."""

    suite: str  # `DEFAULT_SUITE`'s filterset
    seed: str  # the gates' proptest seed
    lint_clippy: list[str]  # the clippy recipe's `tools/lint` line
    hint: str  # the `test` recipe's missing-nextest line


def recipes(makefile: str, ci_yml: str) -> Recipes:
    suite = shlex.split(re.search(r"^DEFAULT_SUITE := (.*)$", makefile, re.M)[1])
    seed = re.search(r"^  PROPTEST_RNG_SEED: .* '(\d+)' ", ci_yml, re.M)[1]
    lint = next(c for c in gate_mirror.makefile_gates(makefile)["clippy"] if LINT + "Cargo.toml" in c)
    hint = re.search(r'echo "(test: cargo-nextest is not installed[^"]*)"', makefile)[1]
    return Recipes(suite[suite.index("-E") + 1], seed, shlex.split(lint), hint)


def matches(path: str, glob: str) -> bool:
    return fuzz_gate.glob_regex(glob).match(path) is not None


def code_read_markdown(ci_yml: str) -> list[str]:
    """The Markdown globs `ci.yml`'s `code` filter counts as code (its `!` line aside)."""
    return [g for g in fuzz_gate.filter_globs(ci_yml, "code") if not g.startswith("!")]


def ci_builds(paths: list[str], ci_yml: str) -> bool:
    """`ci.yml`'s `code` filter: a non-Markdown path, or Markdown it names."""
    read_md = code_read_markdown(ci_yml)
    return any(not p.endswith(".md") or any(matches(p, g) for g in read_md) for p in paths)


def crate_of(path: str, members: dict[str, str]) -> str | None:
    """The workspace member holding `path` (`members`: directory → name), deepest first."""
    for directory in sorted(members, key=len, reverse=True):
        if directory and path.startswith(directory + "/"):
            return members[directory]
    if path.startswith(ROOT_DIRS):
        return ROOT_PACKAGE
    return None


def plan(paths: list[str], members: dict[str, str], ci_yml: str) -> Plan:
    """Which gates `paths` can break. Pure, so the tests need no git or cargo."""
    out = Plan()
    read_md = code_read_markdown(ci_yml)
    crates: set[str] = set()
    for path in sorted(paths):
        why = next((why for glob, why in FALLBACK if matches(path, glob)), None)
        crate = crate_of(path, members)
        if path == "build.rs" or (path.endswith("/build.rs") and path.rsplit("/", 1)[0] in members):
            why = "a build script"
        if why is None and path.endswith(".md"):
            if not any(matches(path, g) for g in read_md):
                continue
            if crate is None:
                why = "Markdown that code outside one crate reads"
        if why is not None:
            return Plan(fallback=f"round-gates cannot place {path}: {why}")
        rust = path.endswith(".rs")
        if crate is not None:
            crates.add(crate)
            out.clippy |= rust
            out.doc |= rust
        elif any(matches(path, g) for g in QUICK_ONLY + CLIPPY_ONLY):
            out.clippy |= path in CLIPPY_ONLY
            out.lint_clippy |= rust and path.startswith(LINT)
        elif path == "deny.toml":
            out.deny = True
        else:
            return Plan(fallback=f"round-gates cannot place {path}: no rule names it")
    out.lint_clippy &= not out.clippy  # `make clippy` runs tools/lint's too
    out.fuzz = bool(fuzz_gate.touched(paths, fuzz_gate.filter_globs(ci_yml)))
    out.code = ci_builds(paths, ci_yml)
    out.crates = sorted(crates)
    return out


def test_commands(crates: list[str], suite: str) -> list[list[str]]:
    """`make test`'s four runs on the workspace's own feature sets, running `crates`' tests."""
    picked = " | ".join(f"package(={c})" for c in crates)
    nextest = ["cargo", "nextest", "run", "--locked", "--no-tests=pass"]
    doctest = ["cargo", "test", "--doc", "--locked"]
    return [
        [*nextest, "-E", f"({suite}) & ({picked})"],
        [*nextest, "--features", "dev", "-E", picked],
        doctest,
        [*doctest, "--features", "dev"],
    ]


def steps(p: Plan, r: Recipes) -> list[tuple[str, list[str]]]:
    """The commands to run, in `make gates`' order."""
    if p.fallback:
        return [("gates", [*MAKE, "gates"])]
    out = [("quick-gates", [*MAKE, "quick-gates"])]
    if p.clippy:
        out.append(("clippy", [*MAKE, "clippy"]))
    if p.lint_clippy:
        out.append(("clippy (tools/lint)", r.lint_clippy))
    if p.doc:
        out.append(("doc", [*MAKE, "doc"]))
    out += [("test", cmd) for cmd in test_commands(p.crates, r.suite)] if p.crates else []
    if p.fuzz:
        out.append(("fuzz-check", [*MAKE, "fuzz-check"]))
    if p.deny:
        out.append(("deny", [*MAKE, "deny"]))
    return out


def summary(p: Plan, head: str, total_crates: int) -> str:
    """The one line for the PR: what ran, what it left to CI, and what CI skips too."""
    if p.fallback:
        return f"Round gates on {head}: make gates in full ({p.fallback}); nothing left to CI that make gates runs."
    others = total_crates - len(p.crates)
    tests = f"the tests of {'the other ' if p.crates else 'all '}{others} workspace packages"
    ran = ["quick-gates"] + [g for g, on in (("clippy", p.clippy), ("tools/lint clippy", p.lint_clippy),
                                              ("doc", p.doc)) if on]
    if p.crates:
        ran.append(f"tests of {', '.join(p.crates)} (default + dev, nextest's package() filter over "
                   "the workspace build, so make test's features) and the workspace's doctests")
    ran += [g for g, on in (("fuzz-check", p.fuzz), ("deny", p.deny)) if on]
    unrun = [g for g, on in (("clippy", p.clippy), ("doc", p.doc)) if not on] + [tests]
    unrun += [] if p.deny else ["deny"]
    left = unrun if p.code else []
    skipped = ([] if p.code else unrun) + ([] if p.fuzz else ["fuzz-check"])
    line = f"Round gates on {head}: ran {'; '.join(ran)}. Left to CI: {', '.join(left) or 'nothing'}."
    return line + (f" Skipped by CI too: {', '.join(skipped)}." if skipped else "")


def workspace() -> tuple[dict[str, str], int] | str:
    """Member directory → name, and the package count, from `cargo metadata`; or why it failed."""
    run = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"],
                         cwd=ROOT, capture_output=True, text=True)
    if run.returncode != 0:
        last = (run.stderr.strip().splitlines() or ["no output"])[-1]
        return f"cargo metadata failed: {last}"
    members = {}
    for pkg in json.loads(run.stdout)["packages"]:
        directory = Path(pkg["manifest_path"]).parent.relative_to(ROOT).as_posix()
        members["" if directory == "." else directory] = pkg["name"]
    return members, len(members)


def shown(cmd: list[str], seed: str) -> str:
    """A command as `--plan` prints it: a cargo test run carries the seed `make` exports."""
    text = shlex.join(cmd)
    return f"PROPTEST_RNG_SEED={seed} {text}" if cmd[:2] in (["cargo", "nextest"], ["cargo", "test"]) else text


def main(argv: list[str]) -> int:
    ci_yml = (ROOT / CI).read_text()
    r = recipes((ROOT / "Makefile").read_text(), ci_yml)
    meta = workspace()
    paths = fuzz_gate.changed_paths()
    if isinstance(meta, str):
        print(f"round-gates: {meta}; running make gates in full.")
        p, total = Plan(fallback=meta), 0
    elif paths is None:
        p, total = Plan(fallback="git cannot say what the branch changes (no origin/main?)"), meta[1]
    else:
        p, total = plan(paths, meta[0], ci_yml), meta[1]
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT,
                          capture_output=True, text=True).stdout.strip() or "HEAD"
    line = summary(p, head, total)
    todo = steps(p, r)
    if "--plan" in argv:
        print("".join(f"  {name}: {shown(cmd, r.seed)}\n" for name, cmd in todo), end="")
        print(line)
        return 0
    if p.crates and shutil.which("cargo-nextest") is None:
        print(r.hint)
        return 1
    for name, cmd in todo:
        print(f"== round-gates: {name}: {shlex.join(cmd)}", flush=True)
        env = {"PROPTEST_RNG_SEED": r.seed, **os.environ}  # `make` exports it; a direct run too
        if subprocess.run(cmd, cwd=ROOT, env=env).returncode != 0:
            print(f"round-gates: {name} failed (above); fix it and run `make round-gates` again.")
            return 1
    print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
