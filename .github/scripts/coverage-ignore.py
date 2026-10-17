#!/usr/bin/env python3
"""The llvm-cov ignore regex that isolates one floored module (#965).

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

    coverage-ignore.py app            # prints the regex for `--ignore-filename-regex`
    coverage-ignore.py --modules      # prints the floored modules, one per line
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

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


def ignore_regex(module: str, modules: list[str], dirs: list[str]) -> str:
    """Ignore every `src/` directory but `module`, top-level files and tools."""
    if module not in modules:
        sys.exit(f"coverage-ignore: `{module}` has no floor in {BASELINE}")
    missing = [m for m in modules if m not in dirs]
    if missing:
        sys.exit(f"coverage-ignore: {BASELINE} floors {missing}, which src/ does not have")
    others = "|".join(d for d in dirs if d != module)
    return rf"src/({others})/|src/[^/]+\.rs|tools/"


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
        print(ignore_regex(args.module, modules, src_dirs(args.root)))
    else:
        parser.error("name a module, or pass --modules")


if __name__ == "__main__":
    main()
