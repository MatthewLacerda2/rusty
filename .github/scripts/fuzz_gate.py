#!/usr/bin/env python3
"""`make fuzz-check` and `make fuzz-lock`: the fuzz target's build gate.

`fuzz/` is its own workspace (cargo-fuzz), so no other gate compiles it and its
target could stop building with nothing red (#2200).

- **`seed`** (`make fuzz-lock`) derives `fuzz/Cargo.lock` from the root
  `Cargo.lock`: a copy, pruned and completed by `cargo metadata` (no compile).
  The lock is derived, so it is gitignored, never committed: nothing drifts and a
  root bump (Dependabot's) needs nothing done here. Only what the fuzz-only crates
  alone pull in is resolved fresh, and `fuzz/Cargo.toml` pins those exactly. A
  shared package the resolve moved off the root's version fails the seed: the
  target would no longer build what the root's gates prove.
- **`check`** (`make fuzz-check`) seeds, then compiles the target on the pinned
  stable toolchain, `--locked` to the seeded lock, into `target/fuzz` so the
  tooling that measures and sweeps `target/` covers it. It is a second cold build
  of the engine, so under `make gates` it runs only when the branch's diff touches
  what can break that build: the paths of `ci.yml`'s `fuzz` filter, read from
  there so the local gate and CI's `fuzz-check` job ask the same question of one
  list. Run by name it always runs. CI's job calls `make fuzz-check` itself, so
  the failure hint exists once, here.

Stdlib only; it shells out to `git` and `cargo`.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FILTER = "fuzz"
MANIFEST = ["--manifest-path", "fuzz/Cargo.toml"]
RESOLVE = ["cargo", "metadata", *MANIFEST, "--format-version", "1"]
CHECK = ["cargo", "check", *MANIFEST, "--locked", "--target-dir", "target/fuzz"]
HINT = (
    "fuzz-check: fuzz/ does not build against the root Cargo.lock's versions.\n"
    "  A compile error: fix fuzz/fuzz_targets/ to the engine's current API (src/lib.rs re-exports it).\n"
    "  A resolve error: fuzz/Cargo.toml asks for a version the root lock does not pin; match it to the root's."
)


def filter_globs(ci_yml: str, name: str = FILTER) -> list[str]:
    """The globs under `name:` in `ci.yml`'s paths-filter `filters: |` block."""
    lines = ci_yml.splitlines()
    head = next((i for i, l in enumerate(lines) if re.fullmatch(rf"\s*{name}:", l)), None)
    if head is None:
        return []
    globs = []
    for line in lines[head + 1 :]:
        item = re.fullmatch(r"\s*- '([^']+)'", line)
        if item:
            globs.append(item[1])
        elif line.strip() and not line.strip().startswith("#"):
            break
    return globs


def glob_regex(glob: str) -> re.Pattern[str]:
    """A paths-filter (picomatch) glob as a regex: `**` crosses `/`, `*` does not."""
    out = re.escape(glob).replace(r"\*\*", "\0").replace(r"\*", "[^/]*").replace("\0", ".*")
    return re.compile(out + r"\Z")


def touched(paths: list[str], globs: list[str]) -> list[str]:
    """The changed paths any of the filter's globs match."""
    patterns = [glob_regex(g) for g in globs]
    return [p for p in paths if any(r.match(p) for r in patterns)]


def changed_paths() -> list[str] | None:
    """What this worktree changes against `origin/main`: commits, edits, new files.

    `None` when git cannot say (no `origin/main`), which runs the check."""
    def git(*args: str) -> list[str] | None:
        run = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
        return run.stdout.split() if run.returncode == 0 else None

    base = git("merge-base", "origin/main", "HEAD")
    if not base:
        return None
    diff = git("diff", "--name-only", base[0])
    new = git("ls-files", "--others", "--exclude-standard")
    return None if diff is None or new is None else diff + new


def lock_versions(text: str) -> dict[str, set[str]]:
    """Each package name a `Cargo.lock` pins and the versions it pins it at."""
    versions: dict[str, set[str]] = {}
    for name, version in re.findall(r'^name = "([^"]+)"\nversion = "([^"]+)"', text, flags=re.M):
        versions.setdefault(name, set()).add(version)
    return versions


def drift(root_lock: str, fuzz_lock: str) -> list[str]:
    """`name version` for each fuzz-lock pin of a root package at a version the root lacks."""
    root, fuzz = lock_versions(root_lock), lock_versions(fuzz_lock)
    return sorted(f"{n} {v} (root: {', '.join(sorted(root[n]))})"
                  for n in fuzz.keys() & root.keys() for v in fuzz[n] - root[n])


def seed() -> int:
    """Derive `fuzz/Cargo.lock` from the root lock; 1 when it cannot keep the root's versions."""
    root = (ROOT / "Cargo.lock").read_text()
    (ROOT / "fuzz/Cargo.lock").write_text(root)
    if subprocess.run(RESOLVE, cwd=ROOT, stdout=subprocess.DEVNULL).returncode != 0:
        return 1
    moved = drift(root, (ROOT / "fuzz/Cargo.lock").read_text())
    if not moved:
        return 0
    print("fuzz-lock: resolving fuzz/ moved packages off the root Cargo.lock's versions:")
    print("".join(f"  {line}\n" for line in moved), end="")
    print("  A fuzz-only dependency needs newer versions than the engine pins: pin it in fuzz/Cargo.toml"
          " to one the root lock satisfies.")
    return 1


def check(if_changed: bool) -> int:
    if if_changed:
        paths = changed_paths()
        hits = None if paths is None else touched(paths, filter_globs((ROOT / ".github/workflows/ci.yml").read_text()))
        if hits == []:
            print(f"fuzz-check: skipped, nothing in this branch's diff matches ci.yml's `{FILTER}` filter "
                  "(run `make fuzz-check` to build it anyway).")
            return 0
    if seed() == 0 and subprocess.run(CHECK, cwd=ROOT).returncode == 0:
        return 0
    print(HINT)
    return 1


def main(argv: list[str]) -> int:
    if argv[:1] == ["seed"]:
        return seed()
    if argv[:1] == ["check"]:
        return check("--if-changed" in argv[1:])
    print("usage: fuzz_gate.py seed | check [--if-changed]")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
