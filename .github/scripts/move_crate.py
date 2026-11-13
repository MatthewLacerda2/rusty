#!/usr/bin/env python3
"""`make move-crate MODULE=<m>`: the mechanical half of moving a module into its crate.

The engine is split one crate per module, over many pull requests
(`crates/CLAUDE.md` is the recipe). Most of a move is the same edits every time,
so this makes them, and lists what it cannot decide:

- `git mv src/<m> crates/<m>/src`, its `mod.rs` becoming `lib.rs`;
- in the crate's files: `crate::<m>::` → `crate::`, `crate::<dep>::` →
  `rusty_<dep>::` for every other module, the shared test helper
  (`crate::test_temp::dir()` → `rusty_test_support::temp::dir()`), and each
  `src/<m>/` path it names → `crates/<m>/src/`;
- `crates/<m>/Cargo.toml`: the workspace's package fields and lints, the sibling
  crates it now names, and the external crates of the root's
  `[workspace.dependencies]` its code names (by `name::`, as the layer lint counts);
  a `dev` / `editor` feature when its code has `cfg(feature = …)` for one;
- the root: the crate in `[workspace.dependencies]` and `[dependencies]`, the
  feature forwards, and `src/lib.rs`'s `pub mod <m>;` → `pub use rusty_<m> as <m>;`.

What it prints as **by hand** is what a text edit cannot settle: a `crate::{…}`
group (its entries may name several modules), `CARGO_MANIFEST_DIR` (it now names
the crate's folder), `crate::test_temp::lua_path`, and a feature that gates an
optional dependency. Then the compiler names the rest (`pub(crate)` reached from
outside, test hooks); the recipe says what to do with each.

    move_crate.py asset            # make the move, print the by-hand list
    move_crate.py asset --root DIR # in another checkout (the tests' fixtures)
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

import modules as layout

FEATURES = ("dev", "editor")


def known_modules(root: Path) -> set[str]:
    """Every module, moved or not: the folders under `src/` but `bin`, and the moved crates."""
    src = {p.name for p in (root / "src").iterdir() if p.is_dir() and p.name != "bin"}
    return src | set(layout.moved(root))


def rewrite(text: str, module: str, others: set[str]) -> tuple[str, set[str]]:
    """`text` with its module paths rewritten for the crate, and the sibling modules it names."""
    text = text.replace(f"crate::{module}::", "crate::")
    text = text.replace("crate::test_temp::dir()", "rusty_test_support::temp::dir()")
    text = re.sub(rf"(?<![\w/])src/{module}/mod\.rs", f"crates/{module}/src/lib.rs", text)
    text = re.sub(rf"(?<![\w/])src/{module}/", f"crates/{module}/src/", text)
    named = set()

    def sibling(m: re.Match) -> str:
        if m.group(1) not in others:
            return m.group(0)
        named.add(m.group(1))
        return f"rusty_{m.group(1)}::"

    text = re.sub(r"(?<![\w$])crate::(\w+)::", sibling, text)
    named |= {m for m in re.findall(r"(?<!\w)rusty_(\w+)::", text) if m in others}
    return text, named


def by_hand(path: str, text: str) -> list[str]:
    """`path:line: why` for each line a text edit cannot settle."""
    checks = [("crate::{", "a `crate::{…}` group: split it per crate"),
              ("CARGO_MANIFEST_DIR", "names the crate's folder now: `rusty_core::project::checkout()`?"),
              ("crate::test_temp::lua_path", "the root's helper: build it from `rusty_test_support::temp::dir()`")]
    return [f"{path}:{n}: {why}" for n, line in enumerate(text.splitlines(), 1)
            for needle, why in checks if needle in line]


def workspace_deps(manifest: str) -> list[str]:
    """The keys of the root's `[workspace.dependencies]` that are not the workspace's own."""
    section = manifest.split("[workspace.dependencies]", 1)[-1].split("\n[", 1)[0]
    keys = re.findall(r"^([A-Za-z][\w-]*)\s*=", section, re.M)
    return [k for k in keys if not k.startswith("rusty-")]


def externs(texts: list[str], deps: list[str]) -> list[str]:
    """The external crates the code names as `name::` (`-` read as `_`), sorted."""
    code = "\n".join(texts)
    return sorted(d for d in deps if re.search(rf"(?<![\w:]){d.replace('-', '_')}::", code))


def manifest(module: str, siblings: set[str], crates: list[str], features: list[str]) -> str:
    """`crates/<m>/Cargo.toml`."""
    out = [f'[package]\nname = "rusty-{module}"', *(f"{k}.workspace = true" for k in ("version", "edition", "publish"))]
    if features:
        out.append("\n[features]\n" + "\n".join(f"{f} = []" for f in features))
    out.append("\n[lints]\nworkspace = true")
    deps = [f"rusty-{s}" for s in sorted(siblings)] + crates
    if deps:
        out.append("\n[dependencies]\n" + "\n".join(f"{d} = {{ workspace = true }}" for d in deps))
    out.append("\n[dev-dependencies]\nrusty-test-support = { workspace = true }")
    return "\n".join(out) + "\n"


def wire_root(root: Path, module: str, features: list[str]) -> None:
    """The root manifest and `src/lib.rs` learn the crate."""
    cargo = (root / "Cargo.toml").read_text()
    cargo = cargo.replace('rusty-test-support = { path = "crates/test-support" }',
                          f'rusty-{module} = {{ path = "crates/{module}" }}\n'
                          'rusty-test-support = { path = "crates/test-support" }', 1)
    cargo = re.sub(r"^(\[dependencies\]\n(?:#.*\n)*)", rf"\1rusty-{module} = {{ workspace = true }}\n",
                   cargo, count=1, flags=re.M)
    for f in features:
        cargo = re.sub(rf'^{f} = \[', f'{f} = ["rusty-{module}/{f}", ', cargo, count=1, flags=re.M)
    (root / "Cargo.toml").write_text(cargo)
    lib = (root / "src" / "lib.rs").read_text()
    lib = re.sub(rf"(#\[cfg\([^\n]*\)\]\n)?pub mod {module};\n", "", lib, count=1)
    last = list(re.finditer(r"^pub use rusty_\w+ as \w+;\n", lib, re.M))[-1]
    lib = lib[:last.end()] + f"pub use rusty_{module} as {module};\n" + lib[last.end():]
    (root / "src" / "lib.rs").write_text(lib)


def move(root: Path, module: str) -> list[str]:
    """Make the move under `root`; the by-hand list."""
    src, crate = root / "src" / module, root / layout.CRATES / module
    if not src.is_dir() or crate.exists():
        sys.exit(f"move-crate: needs src/{module}/ and no crates/{module}/")
    others = known_modules(root) - {module}
    crate.mkdir(parents=True)
    git = ["git", "-C", str(root), "mv"]
    subprocess.run([*git, f"src/{module}", f"crates/{module}/src"], check=True)
    subprocess.run([*git, f"crates/{module}/src/mod.rs", f"crates/{module}/src/lib.rs"], check=True)
    siblings, texts, hand = set(), [], []
    for path in sorted((crate / "src").rglob("*")):
        if path.suffix not in (".rs", ".md"):
            continue
        text, named = rewrite(path.read_text(), module, others)
        path.write_text(text)
        siblings |= named
        texts.append(text)
        hand += by_hand(str(path.relative_to(root)), text)
    code = "\n".join(texts)
    features = [f for f in FEATURES if f'feature = "{f}"' in code]
    crates = externs(texts, workspace_deps((root / "Cargo.toml").read_text()))
    (crate / "Cargo.toml").write_text(manifest(module, siblings, crates, features))
    wire_root(root, module, features)
    return hand


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("module")
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    hand = move(args.root, args.module)
    print(f"move-crate: src/{args.module}/ is crates/{args.module}/ (rusty-{args.module}).")
    if hand:
        print("by hand:\n" + "\n".join(f"  {h}" for h in hand))
    print("next: `cargo check --workspace --all-targets --features dev` and `--no-default-features`, "
          "then crates/CLAUDE.md, *What the compiler will name*.")


if __name__ == "__main__":
    main()
