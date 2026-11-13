"""Where a moved module's paths point, for `make move-crate` (`move_crate.py`): its `src/<m>/`
paths re-aimed at `crates/<m>/src/`, its relative includes re-aimed from their new folder, and
the files outside the crate still naming `src/<m>/`. Its own module so the move stays under
the size gate."""

from __future__ import annotations

import os
import re
import subprocess
from pathlib import Path

import modules as layout


def paths(text: str, module: str) -> str:
    """`text` with each `src/<m>/` path it names → `crates/<m>/src/` (`mod.rs` → `lib.rs`)."""
    text = re.sub(rf"(?<![\w/])src/{module}/mod\.rs", f"crates/{module}/src/lib.rs", text)
    return re.sub(rf"(?<![\w/])src/{module}/", f"crates/{module}/src/", text)


INCLUDE = re.compile(r'(include_(?:str|bytes)!\(\s*")([^"]+)(")')


def reaim(text: str, was: Path, now: Path, module: str, moved_only: bool = False) -> str:
    """`text`'s relative `include_str!` / `include_bytes!` paths, written from `was`'s
    folder, re-aimed from `now`'s at the file they named, which may itself have moved
    from `src/<m>/` (paths relative to the checkout). `moved_only` leaves every include
    whose file did not move exactly as written (PR #2149's review): the outside pass would
    otherwise respell an include it has no reason to touch (`normpath` folds a `./` or a
    `a/../`), a diff in a file no move asked for."""
    def one(m: re.Match) -> str:
        target = Path(os.path.normpath(was.parent / m.group(2)))
        if target.parts[:2] == ("src", module):
            target = Path(layout.CRATES, module, "src", *target.parts[2:])
        elif moved_only:
            return m.group(0)
        rel = os.path.relpath(target, now.parent).replace(os.sep, "/")
        return f"{m.group(1)}{rel}{m.group(3)}"
    return INCLUDE.sub(one, text)


def reaim_outside(root: Path, module: str) -> list[str]:
    """Rewrite each tracked `.rs` file outside the crate whose relative include named a
    file under `src/<m>/`; the files rewritten."""
    done = subprocess.run(["git", "-C", str(root), "grep", "-l", "-E", r"include_(str|bytes)!", "--", "*.rs",
                           f":(exclude){layout.CRATES}/{module}/**"], capture_output=True, text=True, check=False)
    changed = []
    for rel in done.stdout.split():
        path = root / rel
        text = path.read_text()
        new = reaim(text, Path(rel), Path(rel), module, moved_only=True)
        if new != text:
            path.write_text(new)
            changed.append(rel)
    return changed


CRATE_DIR = re.compile(r"crates/[\w-]+/$")
#: Tests whose `src/<m>/` strings are made-up paths, never files to rewrite: coverage
#: isolation's absolute `/w/rusty/src/<m>/…` and a mutation scope's `+++ b/src/<m>/…` diff
#: header. The wider match lists them for every module; they are fixtures of a module path.
FIXTURES = frozenset({".github/scripts/tests/test_coverage_ignore.py", ".github/scripts/tests/mutants/test_scope.py"})


def outside(root: Path, module: str) -> list[str]:
    """Each tracked file outside the crate still naming `src/<m>/`, with its lines."""
    done = subprocess.run(["git", "-C", str(root), "grep", "-n", "-F", f"src/{module}/", "--", ".",
                           f":(exclude)crates/{module}/**"], capture_output=True, text=True, check=False)
    hits: dict[str, list[str]] = {}
    # `crates/asset/src/audio/` holds `src/audio/` too but names another crate's folder: the
    # `audio` move listed eight such lines (#2141). Only a path after `crates/<name>/` is
    # another crate's: PR #2157's review found a wider lookbehind skipping `../src/<m>/`,
    # `./src/<m>/` and `$(CURDIR)/src/<m>/`, real paths to rewrite.
    own = re.compile(rf"(?<![\w-])src/{module}/")
    for row in done.stdout.splitlines():
        path, line, text = row.split(":", 2)
        if path in FIXTURES:
            continue
        if any(not CRATE_DIR.search(text[:m.start()]) for m in own.finditer(text)):
            hits.setdefault(path, []).append(line)
    return [f"{p}:{','.join(ls)}: names `src/{module}/`: rewrite a file path to `crates/{module}/src/`; "
            "a module path the lint tools read stays" for p, ls in sorted(hits.items())]
