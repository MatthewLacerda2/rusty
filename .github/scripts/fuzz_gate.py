#!/usr/bin/env python3
"""`make fuzz-check` and `make fuzz-lock-sync`: the fuzz target's build gate.

`fuzz/` is its own workspace (cargo-fuzz), so no other gate compiles it and its
target could stop building with nothing red (#2200). Two checks hold it:

- **`check`** (`make fuzz-check`) compiles it on the pinned stable toolchain
  against its committed lock, into `target/fuzz` so the tooling that measures and
  sweeps `target/` covers it. It is a second cold build of the engine, so under
  `make gates` it runs only when the branch's diff touches what can break that
  build: the paths of `ci.yml`'s `fuzz` filter, read from there so the local gate
  and CI's `fuzz-check` job ask the same question of one list. Run by name it
  always runs. CI's job calls `make fuzz-check` itself, so the failure hint
  exists once, here.
- **`sync`** (`make fuzz-lock-sync`, a quick gate) is build-free: every
  package `fuzz/Cargo.lock` pins is at a version the root `Cargo.lock` also
  pins, where the root has that package at all. Dependabot's monthly cargo
  update is lockfile-only and edits the root lock alone; the `fuzz` filter does
  not list it, so without this the fuzz lock would silently keep the old
  versions and a later PR using a bumped API would turn the nightly red. With it
  the Dependabot PR itself fails, naming `make fuzz-lock`.

Stdlib only; `check` shells out to `git` and `cargo`, `sync` reads two files.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FILTER = "fuzz"
CHECK = ["cargo", "check", "--manifest-path", "fuzz/Cargo.toml", "--locked", "--target-dir", "target/fuzz"]
HINT = (
    "fuzz-check: fuzz/ does not build.\n"
    "  A lock error: run `make fuzz-lock` and commit fuzz/Cargo.lock.\n"
    "  A compile error: fix fuzz/fuzz_targets/ to the engine's current API (src/lib.rs re-exports it)."
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


def sync() -> int:
    stale = drift((ROOT / "Cargo.lock").read_text(), (ROOT / "fuzz/Cargo.lock").read_text())
    if not stale:
        return 0
    print("fuzz-lock-sync: fuzz/Cargo.lock pins versions the root Cargo.lock no longer does:")
    print("".join(f"  {line}\n" for line in stale), end="")
    print("  Run `make fuzz-lock` and commit fuzz/Cargo.lock.")
    return 1


def check(if_changed: bool) -> int:
    if if_changed:
        paths = changed_paths()
        hits = None if paths is None else touched(paths, filter_globs((ROOT / ".github/workflows/ci.yml").read_text()))
        if hits == []:
            print(f"fuzz-check: skipped, nothing in this branch's diff matches ci.yml's `{FILTER}` filter "
                  "(run `make fuzz-check` to build it anyway).")
            return 0
    if subprocess.run(CHECK, cwd=ROOT).returncode == 0:
        return 0
    print(HINT)
    return 1


def main(argv: list[str]) -> int:
    if argv[:1] == ["sync"]:
        return sync()
    if argv[:1] == ["check"]:
        return check("--if-changed" in argv[1:])
    print("usage: fuzz_gate.py sync | check [--if-changed]")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
