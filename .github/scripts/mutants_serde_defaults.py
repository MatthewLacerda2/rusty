#!/usr/bin/env python3
"""The `exclude_re` lines of `.cargo/mutants.toml` that skip serde default functions.

A `#[serde(default = "f")]` function returns the value a dropped stored key loads.
That value is a design choice: a test of it can only repeat the literal, and the
behaviour around it (a dropped key loads the component's `Default`) is held once,
for every component, by `components::registry::default_drift`. So cargo-mutants
never mutates these functions.

cargo-mutants matches `exclude_re` against a mutant's name as `--list` prints it
(`crates/a/src/b.rs:16:5: replace default_roughness -> f32 with 0.0`), which carries
the file and the function but no attribute. So the lines are generated from the
source, one per file that defines a free default function, naming exactly those
functions; `tests/mutants/test_serde_defaults.py` fails when the config drifts from
what this prints. A `Type::method` default is left mutated: it is an API of its own.

Run it to print the lines to paste: `python3 .github/scripts/mutants_serde_defaults.py`.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
#: A `#[serde(...)]` attribute, possibly over several lines.
ATTR = re.compile(r"#\[serde\((.*?)\)\]", re.S)
#: The function a `default = "..."` argument names.
DEFAULT = re.compile(r'\bdefault\s*=\s*"([A-Za-z_][\w:]*)"')
#: The prefix every generated line starts with, so the test can tell them apart.
PREFIX = "^crates/"


def is_test_file(path: Path) -> bool:
    """A test module: cargo-mutants never mutates it, so it needs no line."""
    return "tests" in path.parts or path.name == "tests.rs" or path.stem.endswith("_tests")


def defines(text: str, name: str) -> bool:
    """`text` defines a free function `name` taking no arguments."""
    return re.search(rf"\bfn {re.escape(name)}\(\)", text) is not None


def imported_from(text: str, name: str) -> str | None:
    """The last module segment of the `use` that brings `name` into `text`, if any."""
    braced = re.search(rf"\buse\s+([\w:]+)::\{{[^}}]*\b{re.escape(name)}\b", text)
    plain = braced or re.search(rf"\buse\s+([\w:]+)::{re.escape(name)}\s*;", text)
    return plain.group(1).split("::")[-1] if plain else None


def crate_src(path: Path) -> Path:
    """The `crates/<name>/src` directory `path` lives in."""
    parts = path.parts
    at = parts.index("src")
    return Path(*parts[: at + 1])


def resolve(root: Path, site: Path, target: str, texts: dict[Path, str]) -> Path | None:
    """The file defining `target`, named in a `default = ...` in `site`, or None for a
    `Type::method`. A bare name is the site's own; a `module::name` path is found by
    its last module segment among the crate's files that define it."""
    segments = target.split("::")
    if len(segments) > 1 and segments[-2][:1].isupper():
        return None
    name = segments[-1]
    if defines(texts[site], name):
        return site
    src = crate_src(site.relative_to(root))
    found = [p for p, t in texts.items() if p.relative_to(root).is_relative_to(src) and defines(t, name)]
    module = segments[-2] if len(segments) > 1 else imported_from(texts[site], name)
    if module:
        found = [p for p in found if module in (p.stem, p.parent.name)] or found
    if len(found) != 1:
        raise SystemExit(f"mutants_serde_defaults: {site.relative_to(root)} names {target!r},"
                         f" defined in {len(found)} files of its crate; name it by its module path")
    return found[0]


def defaults(root: Path = ROOT) -> dict[str, list[str]]:
    """Each file (relative, `/`-separated) and the free serde default functions it defines."""
    texts = {p: p.read_text() for p in sorted((root / "crates").rglob("*.rs"))}
    out: dict[str, set[str]] = {}
    for site, text in texts.items():
        for attr in ATTR.findall(text):
            for target in DEFAULT.findall(attr):
                home = resolve(root, site, target, texts)
                if home is not None and not is_test_file(home):
                    out.setdefault(home.relative_to(root).as_posix(), set()).add(target.split("::")[-1])
    return {path: sorted(names) for path, names in sorted(out.items())}


def lines(found: dict[str, list[str]]) -> list[str]:
    """One `exclude_re` regex per file: its default functions, by name."""
    return [f"^{re.escape(path)}:\\d+:\\d+: replace ({'|'.join(names)}) -> "
            for path, names in found.items()]


def toml(found: dict[str, list[str]]) -> str:
    """The lines as TOML strings, ready to paste into `exclude_re`."""
    return "\n".join("  " + "'" + line + "'," for line in lines(found))


if __name__ == "__main__":
    print(toml(defaults()))
    sys.exit(0)
