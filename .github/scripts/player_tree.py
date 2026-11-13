#!/usr/bin/env python3
"""`player_tree.py` — the shipped player links no editor, no dev layer, no HTTP client.

A shipped game is the `player` binary built `--no-default-features` (CLAUDE.md,
*The editor is a feature too* and *Dev-only build profile*). Clippy on that
feature set only proves the code compiles without naming egui or `dev`; it never
fails when a manifest links one anyway. While the engine is split into crates
(`crates/CLAUDE.md`), that is one `{ workspace = true }` without its `optional`
away: a member that names egui plainly puts it in every build. So this reads
what the player build resolves, `cargo tree -p rusty --no-default-features -e
normal`, and fails on any crate of [`BANNED`]. No build: about a second.
"""

from __future__ import annotations

import re
import subprocess
import sys

TREE = ["cargo", "tree", "--locked", "-p", "rusty", "--no-default-features", "-e", "normal",
        "--prefix", "none", "-f", "{p}"]
#: What only the editor or the `dev` layer may link, each with why: a name, or a
#: prefix ending in `*`.
BANNED = {
    "egui*": "the editor's UI (`editor` feature)",
    "rusty-editor": "the editor crate (`editor` feature)",
    "rusty-dev": "the agentic dev layer (`dev` feature)",
    "scorsese-providers": "the provider boundary (`dev` feature, docs/providers.md)",
    "ureq": "the providers' HTTP client (`dev` feature): a shipped game makes no network call",
}
NAME = re.compile(r"^(\S+) v\S+")


def crates(output: str) -> set[str]:
    """The crate names in `cargo tree` output."""
    return {m.group(1) for line in output.splitlines() if (m := NAME.match(line.strip()))}


def banned(names: set[str]) -> list[str]:
    """Each banned crate the player resolves, with why, sorted."""
    def hit(name: str, rule: str) -> bool:
        return name.startswith(rule[:-1]) if rule.endswith("*") else name == rule
    return sorted(f"{n}: {why}" for n in names for rule, why in BANNED.items() if hit(n, rule))


def main() -> int:
    names = crates(subprocess.run(TREE, check=True, capture_output=True, text=True).stdout)
    if "rusty" not in names:
        print("player-tree: `cargo tree` listed no `rusty` package; nothing was checked")
        return 1
    found = banned(names)
    for line in found:
        print(f"player-tree: the --no-default-features build links {line}")
    if found:
        print("  A manifest names it without `optional = true` behind its feature, or a crate that")
        print("  needs it is no longer optional: gate it (crates/CLAUDE.md, *Features*).")
        return 1
    print("player-tree: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
