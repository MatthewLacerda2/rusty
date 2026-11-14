#!/usr/bin/env python3
"""A coverage floor is never lowered silently (#963).

`coverage-baseline.txt` holds the per-module line floors the weekly coverage
run ratchets against. The run itself is an informational signal, but the floor
file is not: "never lower one silently to make a table green" was a sentence in
the file's header, `docs/testing.md` and the ci-merge skill, and nothing held
it. A floor that drops (or a floored module that disappears while its
`crates/<module>/src/` is still there) is a finding unless the lowered line carries an
inline reason:

    api 85.00  # reason: the Sound.* adapter moved out of api (#1234)

A reason answers for one drop: lowering a line again while keeping the reason
it already had is still silent. A raise, an unchanged floor and a new floor
always pass. A removed floor can
carry no reason on its line, so it passes only when its module is gone from
`crates/` (`coverage-ignore.py` refuses a floor on a missing crate anyway), or
when it was split into floors on its halves (`script` into `script/api` and
`script/scripting`): every folder the old floor covered then needs a half floor
at least the old value, and a half below it, or a missing one, needs a reason on
a half's line, as a drop does.

The check runs as a `make scripts` test against the merge base with
`origin/main` (`tests/test_coverage_floors.py`), so it holds in `make gates` and
in lint.yml without a step of its own. Run by hand:

    coverage_floors.py [BASE_REF]     # default origin/main; exit 1 on a finding
"""

from __future__ import annotations

import subprocess
import sys

from modules import module_dir
from pathlib import Path
from typing import Callable

BASELINE = "coverage-baseline.txt"
REPO_ROOT = Path(__file__).resolve().parents[2]
REASON = "# reason:"


class GitError(RuntimeError):
    """A `git` call failed (a shallow clone without the merge base, say)."""


def parse(text: str) -> dict[str, tuple[float, str, str]]:
    """Each module's `(floor, floor as written, reason)`; the reason is "" when absent."""
    floors = {}
    for line in text.splitlines():
        body, _, reason = line.partition(REASON)
        words = body.split("#", 1)[0].split()
        if len(words) >= 2:
            floors[words[0]] = (float(words[1]), words[1], reason.strip())
    return floors


def findings(base: str, head: str, folder: Callable[[str], str | None],
             subfolders: Callable[[str], list[str]] = lambda m: []) -> list[str]:
    """Every floor `head` lowers from `base` without a reason, as a message.
    `folder(m)` is where module `m`'s code lives (`None` once gone), and
    `subfolders(m)` the folders that holds, which a split floor must all cover."""
    old, new = parse(base), parse(head)
    out = []
    for module, (floor, written, old_reason) in old.items():
        if module not in new:
            out += removed(module, floor, written, new, folder, subfolders)
            continue
        now, now_written, reason = new[module]
        if now < floor and (not reason or reason == old_reason):
            out.append(
                f"{BASELINE}: `{module}` floor dropped {written} -> {now_written} without "
                f"a new reason; raise coverage instead, or end the line with `{REASON} <why>`"
            )
    return out


def removed(module: str, floor: float, written: str, new: dict[str, tuple[float, str, str]],
            folder: Callable[[str], str | None],
            subfolders: Callable[[str], list[str]]) -> list[str]:
    """Why removing `module`'s floor is silent: its folder still exists and no
    half floor stands in its place, or the halves leave a folder of it unfloored
    or floor one below the old value, with no reason on any half's line."""
    where = folder(module)
    if where is None:
        return []
    halves = {name: v for name, v in new.items() if name.startswith(f"{module}/")}
    if not halves:
        return [f"{BASELINE}: the `{module}` floor ({written}) was removed while "
                f"{where}/ still exists; keep it, or lower it with `{REASON} <why>`"]
    if any(reason for _, _, reason in halves.values()):
        return []
    out = []
    for sub in subfolders(module):
        half = f"{module}/{sub}"
        if half not in halves:
            out.append(f"{BASELINE}: the `{module}` floor ({written}) was split, but "
                       f"{where}/{sub}/ has no `{half}` floor; floor it at {written} or more, "
                       f"or end a half's line with `{REASON} <why>`")
        elif halves[half][0] < floor:
            out.append(f"{BASELINE}: the `{module}` floor ({written}) was split into `{half}` "
                       f"at {halves[half][1]}, below it; floor it at {written} or more, "
                       f"or end its line with `{REASON} <why>`")
    return out


def git(*args: str, cwd: Path) -> str:
    done = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if done.returncode != 0:
        raise GitError(f"git {' '.join(args)}: {done.stderr.strip()}")
    return done.stdout


def against(base_ref: str, root: Path) -> list[str]:
    """`findings` for the working tree against its merge base with `base_ref`."""
    merge_base = git("merge-base", "HEAD", base_ref, cwd=root).strip()
    base = git("show", f"{merge_base}:{BASELINE}", cwd=root)
    head = (root / BASELINE).read_text()
    def path(m: str) -> Path:
        crate, _, sub = m.partition("/")
        return module_dir(root, crate) / sub

    def folder(m: str) -> str | None:
        return path(m).relative_to(root).as_posix() if path(m).is_dir() else None

    def subfolders(m: str) -> list[str]:
        return sorted(p.name for p in path(m).iterdir() if p.is_dir())

    return findings(base, head, folder, subfolders)


def main() -> None:
    base_ref = sys.argv[1] if len(sys.argv) > 1 else "origin/main"
    try:
        found = against(base_ref, REPO_ROOT)
    except GitError as err:
        sys.exit(f"coverage floors: {err}")
    print("\n".join(found) or f"coverage floors: none lowered silently against {base_ref}")
    sys.exit(1 if found else 0)


if __name__ == "__main__":
    main()
