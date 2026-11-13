"""Where a top-level module's code lives while the engine is split into crates.

`src/<m>/` until the module moves, `crates/<m>/src/` after, with
`crates/<m>/src/lib.rs` in the place of `src/<m>/mod.rs` (crates/CLAUDE.md).
A script that scopes by module reads a path at its **module path**, the
`src/<m>/…` spelling it has wherever it lives, so one table row holds a module
before and after its move. The mirror of `tools/lint/src/modules.rs`, which the
scripts cannot call; keep the two in step.
"""

from __future__ import annotations

from pathlib import Path

CRATES = "crates"
#: Members that are no module of the layer table, each with why.
NOT_MODULES = {"test-support": "the in-crate tests' shared helpers, only ever a dev-dependency"}


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


def moved(root: Path) -> list[str]:
    """The modules already moved under `root`: every member of `crates/` with a
    `src/` folder but the [`NOT_MODULES`], sorted."""
    crates = root / CRATES
    if not crates.is_dir():
        return []
    return sorted(p.name for p in crates.iterdir() if (p / "src").is_dir() and p.name not in NOT_MODULES)


def module_dir(root: Path, module: str) -> Path:
    """The folder that holds `module`'s code: `crates/<m>/src` once moved, else `src/<m>`."""
    crate = root / CRATES / module / "src"
    return crate if crate.is_dir() else root / "src" / module
