#!/usr/bin/env python3
r"""The llvm-cov ignore regex that isolates one floored module (#965).

`coverage.yml`'s ratchet measures each module named in `coverage-baseline.txt`
on its own. The `regex` crate llvm-cov uses has no look-around, so a module is
isolated by ignoring everything else: every other `src/<dir>/`, every top-level
`src/*.rs` (main/lib/settings/…, the #163 class of bug) and `tools/`.

Both lists used to be typed into the workflow by hand, and a new `src/<dir>/`
missing from them leaked into every floored number (audio, procgen, shadergen,
then preview). Here they are derived: the floored modules are the baseline's
names, and every other directory under `src/` is ignored, so a new module is
excluded the day it lands. A baseline name with no `src/<name>/` is refused:
a floor on nothing would report the whole crate as that module.

While the engine is split into crates (#2139, crates/CLAUDE.md), a module is
`src/<m>/` or `crates/<m>/src/`, so every other member under `crates/` is ignored
too, and the root package's top-level files are named one by one and anchored
to the checkout's absolute path (llvm-cov reports absolute file names), so a
moved crate's own `lib.rs` is not read as the root's `src/lib.rs`. A moved
module with a subfolder named like a root module is refused: its `src/<name>/`
would read as that module and be ignored.

A floor may name a sub-path of a moved crate, `<crate>/<folder>` (`script/api`):
a crate that holds two modules floors each half on its own, so one half's slack
never hides the other's drop. It is measured as its crate, with every other
entry of `crates/<crate>/src/` ignored too (so the crate's own top-level files
count toward neither half).

    coverage-ignore.py app            # prints the regex for `--ignore-filename-regex`
    coverage-ignore.py --modules      # prints the floored modules, one per line
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import modules as layout

BASELINE = "coverage-baseline.txt"


def floored(baseline: str) -> list[str]:
    """The module names in the baseline, in order; `#` lines and blanks skipped."""
    names = []
    for line in baseline.splitlines():
        words = line.split()
        if words and not words[0].startswith("#"):
            names.append(words[0])
    return names


def src_dirs(root: Path) -> list[str]:
    """Every directory directly under `src/`."""
    return sorted(p.name for p in (root / "src").iterdir() if p.is_dir())


def src_files(root: Path) -> list[str]:
    """Every `.rs` file directly under `src/` (the root package's crate roots and helpers)."""
    return sorted(p.name for p in (root / "src").glob("*.rs"))


def members(root: Path) -> list[str]:
    """Every member folder under `crates/`, modules and helpers alike."""
    crates = root / layout.CRATES
    return sorted(p.name for p in crates.iterdir() if p.is_dir()) if crates.is_dir() else []


def entries(root: Path, crate: str) -> list[str]:
    """What `crates/<crate>/src/` holds: folders as `name/`, files as `name`."""
    inner = root / layout.CRATES / crate / "src"
    if not inner.is_dir():
        return []
    return sorted(p.name + "/" if p.is_dir() else p.name for p in inner.iterdir())


def ignore_regex(module: str, modules: list[str], dirs: list[str], files: list[str] = (),
                 crates: list[str] = (), root: Path | None = None,
                 inside: list[str] = ()) -> str:
    """Ignore every module but `module` (under `src/` or `crates/`), the root
    package's top-level files and tools. A `<crate>/<folder>` floor also ignores
    every other entry of the crate's `src/`, `inside` being what that holds."""
    if module not in modules:
        sys.exit(f"coverage-ignore: `{module}` has no floor in {BASELINE}")
    missing = [m for m in modules if m.partition("/")[0] not in dirs
               and m.partition("/")[0] not in crates]
    if missing:
        sys.exit(f"coverage-ignore: {BASELINE} floors {missing}, which neither src/ nor crates/ has")
    crate, _, sub = module.partition("/")
    if sub and (crate not in crates or f"{sub}/" not in inside):
        sys.exit(f"coverage-ignore: `{module}` floors a folder crates/{crate}/src/ does not hold")
    others = [d for d in dirs if d != crate]
    if crate in crates and root is not None:
        inner = layout.module_dir(root, crate)
        clash = sorted(p.name for p in inner.iterdir() if p.is_dir() and p.name in others)
        if clash:
            sys.exit(f"coverage-ignore: crates/{crate}/src/ has {clash}, named like root modules; "
                     "its lines would be ignored as theirs: rename the folder")
    parts = [f"src/({'|'.join(others)})/"] if others else []
    stems = [re.escape(f) for f in files] or [r"[^/]+\.rs"]
    # Anchored at the checkout when it is known: unanchored, `src/lib.rs` also
    # matches `crates/<m>/src/lib.rs`, and the isolated crate would lose its own.
    anchor = f"^{re.escape(root.resolve().as_posix())}/" if root is not None else ""
    parts.append(f"{anchor}src/({'|'.join(stems)})")
    if rest := [c for c in crates if c != crate]:
        parts.append(f"crates/({'|'.join(rest)})/")
    if sub:
        siblings = [re.escape(e) for e in inside if e != f"{sub}/"]
        parts.append(f"crates/{re.escape(crate)}/src/({'|'.join(siblings)})")
    parts.append("tools/")
    return "|".join(parts)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("module", nargs="?", help="the floored module to isolate")
    parser.add_argument("--modules", action="store_true", help="list the floored modules")
    parser.add_argument("--root", type=Path, default=Path("."), help="the repository root")
    args = parser.parse_args()
    modules = floored((args.root / BASELINE).read_text())
    if args.modules:
        print("\n".join(modules))
    elif args.module:
        crate = args.module.partition("/")[0]
        print(ignore_regex(args.module, modules, src_dirs(args.root), src_files(args.root),
                           members(args.root), args.root, entries(args.root, crate)))
    else:
        parser.error("name a module, or pass --modules")


if __name__ == "__main__":
    main()
