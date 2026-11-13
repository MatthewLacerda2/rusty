"""What a module's code names, read as text for `make move-crate` (`move_crate.py`): which
files only tests compile, its code without `#[cfg(test)]` items, and the external crates it
names. Its own module so the move itself stays under the size gate (the seed move of #2140
tipped it over)."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path


def is_test_file(rel: Path) -> bool:
    """A file only tests compile, by the repo's naming: `tests.rs`, `*_tests.rs`, under `tests/`."""
    return rel.name == "tests.rs" or rel.name.endswith("_tests.rs") or "tests" in rel.parts[:-1]


def live(text: str) -> str:
    """`text` without its `#[cfg(test)]` items (a coarse line scan, like the layer lint's)."""
    out, lines, at = [], text.splitlines(), 0
    while at < len(lines):
        line = lines[at]
        if not line.strip().startswith("#[cfg(test)]"):
            out.append(line)
            at += 1
            continue
        depth, opened = 0, False
        rest = line.strip()[len("#[cfg(test)]"):]
        while True:
            depth += rest.count("{") - rest.count("}")
            opened |= "{" in rest
            if (opened and depth <= 0) or (not opened and rest.rstrip().endswith(";")):
                break
            at += 1
            if at >= len(lines):
                break
            rest = lines[at]
        at += 1
    return "\n".join(out)


def workspace_deps(manifest: str) -> list[str]:
    """The keys of the root's `[workspace.dependencies]` that are not the workspace's own."""
    keys = tomllib.loads(manifest).get("workspace", {}).get("dependencies", {})
    return [k for k in keys if not k.startswith("rusty-")]


def shadowed(code: str) -> set[str]:
    """The names a file declares or imports as its own (`mod image;`, `use super::image;`,
    `use crate::{a, image}`, `… as image`), whose `name::` then means the local item, not a
    crate: the rule `tools/lint/src/layers/externs/find.rs` scans by."""
    names = set(re.findall(r"^\s*(?:pub(?:\([\w:]+\))?\s+)?mod\s+(\w+)", code, re.M))
    names |= set(re.findall(r"\bas\s+(\w+)\s*[;,}]", code))
    for path in re.findall(r"\buse\s+((?:crate|super|self)\b[^;]*);", code):
        names |= {seg.strip().split("::")[-1] for seg in re.split(r"[{},]", path) if seg.strip()}
    return names - {"self", "super", "crate", "*"}


def externs(code: str | list[str], deps: list[str]) -> list[str]:
    """The external crates `code` (one file's text, or each file's) names as `name::` (`-` read
    as `_`), sorted. In each file, a name it declares as its own (`shadowed`) is no crate."""
    files = [code] if isinstance(code, str) else code
    return sorted(d for d in deps if any(
        d.replace("-", "_") not in shadowed(f) and re.search(rf"(?<![\w:]){d.replace('-', '_')}::", f)
        for f in files))
