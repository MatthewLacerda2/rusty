"""Where a top-level module's code lives: each is a workspace member,
`crates/<m>/src/`, whose `lib.rs` is the module's root (crates/CLAUDE.md).

A script that scopes by module reads a path at its **module path**, `src/<m>/…`
(`crates/<m>/src/lib.rs` read as `src/<m>/mod.rs`): the spelling `make
review-level`'s table, the `stored-format` markers and the lint tool's rules and
layer table share. The mirror of `tools/lint/src/modules.rs`, which the scripts
cannot call; keep the two in step: [`drift`] holds their `NOT_MODULES` equal
(`tests/modules/test_modules.py`).
"""

from __future__ import annotations

import re
from pathlib import Path

CRATES = "crates"
#: Members that are no module of the layer table, each with why.
NOT_MODULES = {"test-support": "the in-crate tests' shared helpers, only ever a dev-dependency"}


#: The Rust mirror whose `NOT_MODULES` must match this one's.
RUST_MIRROR = "tools/lint/src/modules.rs"


def drift(rust: str) -> str | None:
    """How the Rust mirror's `NOT_MODULES` (its source, `rust`) differs from this
    one's, or `None` when they name the same members."""
    table = re.search(r"NOT_MODULES[^=]*=\s*&\[(.*?)\];", rust, re.S)
    theirs = set(re.findall(r'\(\s*"([^"]+)"', table.group(1))) if table else set()
    ours = set(NOT_MODULES)
    if theirs == ours:
        return None
    return (f"NOT_MODULES differ: only in {RUST_MIRROR}: {sorted(theirs - ours)}, "
            f"only in .github/scripts/modules.py: {sorted(ours - theirs)}; list a member in both")


def crate_module(path: str) -> str | None:
    """The module a `crates/<m>/…` path belongs to, unless `<m>` is no module."""
    parts = path.split("/")
    if len(parts) < 2 or parts[0] != CRATES or not parts[1] or parts[1] in NOT_MODULES:
        return None
    return parts[1]


def module_path(path: str) -> str:
    """`crates/<m>/src/lib.rs` → `src/<m>/mod.rs`, `crates/<m>/src/<rest>` →
    `src/<m>/<rest>`; any other path unchanged."""
    module = crate_module(path)
    if module is None:
        return path
    inner = path[len(CRATES) + 1 + len(module):]
    if inner in ("/src", "/src/"):
        return f"src/{module}" + inner[4:]
    if not inner.startswith("/src/"):
        return path
    rest = inner[len("/src/"):]
    return f"src/{module}/mod.rs" if rest == "lib.rs" else f"src/{module}/{rest}"


def module_dir(root: Path, module: str) -> Path:
    """The folder that holds `module`'s code: `crates/<m>/src`."""
    return root / CRATES / module / "src"
