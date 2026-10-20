#!/usr/bin/env python3
"""A coverage floor is never lowered silently (#963).

`coverage-baseline.txt` holds the per-module line floors the weekly coverage
run ratchets against. The run itself is an informational signal, but the floor
file is not: "never lower one silently to make a table green" was a sentence in
the file's header, `docs/testing.md` and the ci-merge skill, and nothing held
it. A floor that drops (or a floored module that disappears while its
`src/<module>/` is still there) is a finding unless the lowered line carries an
inline reason:

    api 85.00  # reason: the Sound.* adapter moved out of api (#1234)

A raise, an unchanged floor and a new floor always pass. A removed floor can
carry no reason on its line, so it passes only when its module is gone from
`src/` (`coverage-ignore.py` refuses a floor on a missing directory anyway).

The check runs as a `make scripts` test against the merge base with
`origin/main` (`tests/test_coverage_floors.py`), so it holds in `make gates` and
in lint.yml without a step of its own. Run by hand:

    coverage_floors.py [BASE_REF]     # default origin/main; exit 1 on a finding
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from typing import Callable

BASELINE = "coverage-baseline.txt"
REPO_ROOT = Path(__file__).resolve().parents[2]
REASON = "# reason:"


def parse(text: str) -> dict[str, tuple[float, str]]:
    """Each floored module's `(floor, reason)`; the reason is "" when absent."""
    floors = {}
    for line in text.splitlines():
        body, _, reason = line.partition(REASON)
        words = body.split("#", 1)[0].split()
        if len(words) >= 2:
            floors[words[0]] = (float(words[1]), reason.strip())
    return floors


def findings(base: str, head: str, has_module: Callable[[str], bool]) -> list[str]:
    """Every floor `head` lowers from `base` without a reason, as a message."""
    old, new = parse(base), parse(head)
    out = []
    for module, (floor, _) in old.items():
        if module not in new:
            if has_module(module):
                out.append(
                    f"{BASELINE}: the `{module}` floor ({floor:.2f}) was removed while "
                    f"src/{module}/ still exists; keep it, or lower it with `{REASON} <why>`"
                )
            continue
        now, reason = new[module]
        if now < floor and not reason:
            out.append(
                f"{BASELINE}: `{module}` floor dropped {floor:.2f} -> {now:.2f} without "
                f"a reason; raise coverage instead, or end the line with `{REASON} <why>`"
            )
    return out


def git(*args: str, cwd: Path) -> str:
    return subprocess.run(
        ["git", *args], cwd=cwd, check=True, capture_output=True, text=True
    ).stdout


def against(base_ref: str, root: Path) -> list[str]:
    """`findings` for the working tree against its merge base with `base_ref`."""
    merge_base = git("merge-base", "HEAD", base_ref, cwd=root).strip()
    base = git("show", f"{merge_base}:{BASELINE}", cwd=root)
    head = (root / BASELINE).read_text()
    return findings(base, head, lambda m: (root / "src" / m).is_dir())


def main() -> None:
    base_ref = sys.argv[1] if len(sys.argv) > 1 else "origin/main"
    found = against(base_ref, REPO_ROOT)
    print("\n".join(found) or f"coverage floors: none lowered silently against {base_ref}")
    sys.exit(1 if found else 0)


if __name__ == "__main__":
    main()
