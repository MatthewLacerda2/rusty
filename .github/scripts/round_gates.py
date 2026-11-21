#!/usr/bin/env python3
"""`make round-gates`: the gates of what a review round touched (#2452).

A `Round:` session (a re-brief on a pushed branch) used to pay the full `make
gates`, about 25 minutes from an empty `target/`, to ready a ten-line fix: the
gates were 61% of a round's time (`make coder-cost`). The operator chose to run
only the gates a round's files can break; the pull request's CI and the merge
queue's tested tree still run everything before anything merges, so a round that
readies red is handed back as any red PR is.

What runs, from the files the branch changes against its merge base with
`origin/main` (committed, edited and new, as `fuzz_gate.changed_paths` reads them):

- **`quick-gates`, always.** Seconds, and the only gates a docs-only diff gets.
- **`clippy`** when any `.rs` changed (the whole workspace and `tools/lint`): it
  compiles every crate's targets, so a signature change that breaks a dependent's
  code or tests is caught here even though only the changed crates' tests run.
- **The tests of each workspace crate that holds a changed file**, as `make test`
  runs them: nextest without the GPU group and with it under `dev`, then the
  doctests, each crate on the `editor` and `dev` features it has (the root's
  `editor`/`dev` turn on the members' own). The crate of a path is the deepest
  member directory `cargo metadata` lists above it; no list is kept here. The root
  package holds `src/` and `tests/`, so the integration binary runs only when one
  of those changed.
- **`doc`** when any `.rs` of the workspace changed; **`fuzz-check`** when the
  diff matches `ci.yml`'s `fuzz` filter (what `make gates` asks too); **`deny`**
  when `deny.toml` changed.

A change it cannot place falls back to the full `make gates` and says so: the
`Makefile`, a workflow or action, `Cargo.lock`, a manifest (it moves features
and dependencies across the graph, and `deny` and `fuzz-check` read it), a build
script, the toolchain or cargo/nextest config, Markdown that code reads (`ci.yml`'s
`code` filter: `docs/api/` is read by tests in several crates), and any path
outside a crate that no rule here names (`engine/`, `games/`: read by many crates).

It ends with one line a coder pastes into the PR under the round's heading: what
ran, on which commit, and what it left to CI. `--plan` prints that line and the
commands, and runs nothing. Stdlib only.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

import fuzz_gate

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
]
# Paths whose every gate is in `quick-gates` (their `.rs`, `tools/lint`'s, also gets clippy).
QUICK_ONLY = [".github/scripts/**", "tools/**", ".claude/**", ".githooks/**", "rustfmt.toml", "fuzz/**"]
CLIPPY_ONLY = ["clippy.toml"]
DEFAULT_SUITE = "not group(gpu*)"
FEATURES = ("editor", "dev")
HEAVY = ("clippy", "test", "doc", "fuzz-check", "deny")


@dataclass
class Plan:
    """What a round runs. `fallback` set means `make gates` in full, for that reason."""

    fallback: str | None = None
    clippy: bool = False
    doc: bool = False
    fuzz: bool = False
    deny: bool = False
    crates: list[str] = field(default_factory=list)


def matches(path: str, glob: str) -> bool:
    return fuzz_gate.glob_regex(glob).match(path) is not None


def code_read_markdown(ci_yml: str) -> list[str]:
    """The Markdown globs `ci.yml`'s `code` filter counts as code (its `!` line aside)."""
    return [g for g in fuzz_gate.filter_globs(ci_yml, "code") if not g.startswith("!")]


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
        rust = path.endswith(".rs") and not path.startswith("fuzz/")  # fuzz/ is its own workspace
        out.clippy |= rust
        if crate is not None:
            crates.add(crate)
            out.doc |= rust
        elif any(matches(path, g) for g in QUICK_ONLY + CLIPPY_ONLY):
            out.clippy |= path in CLIPPY_ONLY
        elif path == "deny.toml":
            out.deny = True
        else:
            return Plan(fallback=f"round-gates cannot place {path}: no rule names it")
    out.fuzz = bool(fuzz_gate.touched(paths, fuzz_gate.filter_globs(ci_yml)))
    out.crates = sorted(crates)
    return out


def test_commands(crates: list[str], features: dict[str, list[str]]) -> list[list[str]]:
    """`make test`'s four runs, narrowed to `crates`, each on the features it has."""
    picks = ["-p" if i % 2 == 0 else c for c in crates for i in range(2)]

    def on(wanted: tuple[str, ...]) -> list[str]:
        named = [f"{c}/{f}" for c in crates for f in wanted if f in features.get(c, [])]
        return ["--features", ",".join(named)] if named else []

    nextest = ["cargo", "nextest", "run", "--locked", "--no-tests=pass", *picks]
    doctest = ["cargo", "test", "--doc", "--locked", *picks]
    return [
        [*nextest, *on(FEATURES[:1]), "-E", DEFAULT_SUITE],
        [*nextest, *on(FEATURES)],
        [*doctest, *on(FEATURES[:1])],
        [*doctest, *on(FEATURES)],
    ]


def steps(p: Plan, features: dict[str, list[str]]) -> list[tuple[str, list[str]]]:
    """The commands to run, in `make gates`' order."""
    if p.fallback:
        return [("gates", ["make", "--no-print-directory", "gates"])]
    out = [("quick-gates", ["make", "--no-print-directory", "quick-gates"])]
    if p.clippy:
        out.append(("clippy", ["make", "--no-print-directory", "clippy"]))
    if p.doc:
        out.append(("doc", ["make", "--no-print-directory", "doc"]))
    out += [("test", cmd) for cmd in test_commands(p.crates, features)] if p.crates else []
    if p.fuzz:
        out.append(("fuzz-check", ["make", "--no-print-directory", "fuzz-check"]))
    if p.deny:
        out.append(("deny", ["make", "--no-print-directory", "deny"]))
    return out


def summary(p: Plan, head: str, total_crates: int) -> str:
    """The one line for the PR: what ran and what it left to CI."""
    if p.fallback:
        return f"Round gates on {head}: make gates in full ({p.fallback}); nothing left to CI that make gates runs."
    ran = ["quick-gates"] + [g for g, on in (("clippy", p.clippy), ("doc", p.doc)) if on]
    left = [g for g, on in (("clippy", p.clippy), ("doc", p.doc)) if not on]
    if p.crates:
        ran.append(f"tests of {', '.join(p.crates)} (default + dev, doctests)")
    others = total_crates - len(p.crates)
    left.append(f"the tests of {'the other ' if p.crates else 'all '}{others} workspace packages")
    ran += [g for g, on in (("fuzz-check", p.fuzz), ("deny", p.deny)) if on]
    left += [g for g, on in (("fuzz-check", p.fuzz), ("deny", p.deny)) if not on]
    return f"Round gates on {head}: ran {'; '.join(ran)}. Left to CI: {', '.join(left)}."


def workspace() -> tuple[dict[str, str], dict[str, list[str]]]:
    """Member directory → name, and name → features, from `cargo metadata`."""
    meta = json.loads(subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=ROOT, capture_output=True, text=True, check=True).stdout)
    members, features = {}, {}
    for pkg in meta["packages"]:
        directory = Path(pkg["manifest_path"]).parent.relative_to(ROOT).as_posix()
        members["" if directory == "." else directory] = pkg["name"]
        features[pkg["name"]] = sorted(pkg["features"])
    return members, features


def main(argv: list[str]) -> int:
    members, features = workspace()
    paths = fuzz_gate.changed_paths()
    ci_yml = (ROOT / CI).read_text()
    p = Plan(fallback="git cannot say what the branch changes (no origin/main?)") if paths is None \
        else plan(paths, members, ci_yml)
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT,
                          capture_output=True, text=True).stdout.strip() or "HEAD"
    line = summary(p, head, len(members))
    todo = steps(p, features)
    if "--plan" in argv:
        print("".join(f"  {name}: {' '.join(cmd)}\n" for name, cmd in todo), end="")
        print(line)
        return 0
    for name, cmd in todo:
        print(f"== round-gates: {name}: {' '.join(cmd)}", flush=True)
        if subprocess.run(cmd, cwd=ROOT).returncode != 0:
            print(f"round-gates: {name} failed (above); fix it and run `make round-gates` again.")
            return 1
    print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
