#!/usr/bin/env python3
"""`make move-crate MODULE=<m>`: the mechanical half of moving a module into its crate.

The engine is split one crate per module, over many pull requests
(`crates/CLAUDE.md` is the recipe). Most of a move is the same edits every time,
so this makes them, and lists what it cannot decide:

- `git mv src/<m> crates/<m>/src`, its `mod.rs` becoming `lib.rs`;
- in the crate's files: `crate::<dep>::` → `rusty_<dep>::` for every other
  top-level module, then `crate::<m>::` → `crate::` (in that order, so a
  submodule named like a module, `crate::render::scene::`, stays the crate's
  own), the shared test helper (`crate::test_temp::dir()` →
  `rusty_test_support::temp::dir()`), and each `src/<m>/` path it names →
  `crates/<m>/src/`;
- `crates/<m>/Cargo.toml`: the workspace's package fields and lints, the sibling
  crates it now names, and the external crates of the root's
  `[workspace.dependencies]` its code names (by `name::`, as the layer lint
  counts): those its non-test code names in `[dependencies]` (or the root's
  `[target.…]` table they sit in), those only its tests name in
  `[dev-dependencies]`;
- features: a dependency the root declares `optional` is optional in the crate
  too, enabled by the crate's feature of the same name as the root's
  (`dev = ["dep:x"]`); a sibling with a `dev` / `editor` feature has it
  forwarded (`"rusty-y/dev"`); a `dev` / `editor` the code tests with
  `cfg(feature = …)` is declared;
- the root: the crate in `[workspace.dependencies]` and `[dependencies]`, the
  feature forwards, and `src/lib.rs`'s `pub mod <m>;` → `pub use rusty_<m> as <m>;`.
  A `pub mod` gated by `#[cfg(feature = "f")]` stays gated: the root depends on
  the crate optionally, its feature `f` turns it on (`"dep:rusty-<m>"`), and the
  re-export carries the same `cfg`.

What it prints as **by hand** is what a text edit cannot settle: a `crate::{…}`
group (its entries may name several modules), `CARGO_MANIFEST_DIR` (it now names
the crate's folder), `crate::test_temp::lua_path`, an optional dependency no one
feature of `dev` / `editor` turns on, a feature other than those two, a `pub mod`
gated by anything but one feature, and every tracked file outside the crate that
still names `src/<m>/` (a file path to rewrite, or a module path the lint tools
read, which stays). Then the compiler names the rest (`pub(crate)` reached from
outside, test hooks); the recipe says what to do with each.

    move_crate.py asset            # make the move, print the by-hand list
    move_crate.py asset --root DIR # in another checkout (the tests' fixtures)
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path

import modules as layout

FEATURES = ("dev", "editor")


def quote(item: str) -> str:
    """`item` as a TOML string."""
    return f'"{item}"'


def known_modules(root: Path) -> set[str]:
    """Every module, moved or not: the folders under `src/` but `bin`, and the moved crates."""
    src = {p.name for p in (root / "src").iterdir() if p.is_dir() and p.name != "bin"}
    return src | set(layout.moved(root))


def rewrite(text: str, module: str, others: set[str]) -> tuple[str, set[str]]:
    """`text` with its module paths rewritten for the crate, and the sibling modules it names."""
    named = set()

    def sibling(m: re.Match) -> str:
        if m.group(1) not in others:
            return m.group(0)
        named.add(m.group(1))
        return f"rusty_{m.group(1)}::"

    # Siblings first: `crate::<m>::scene::` must not read as the sibling `scene`.
    text = re.sub(r"(?<![\w$])crate::(\w+)::", sibling, text)
    text = text.replace(f"crate::{module}::", "crate::")
    text = text.replace("crate::test_temp::dir()", "rusty_test_support::temp::dir()")
    text = re.sub(rf"(?<![\w/])src/{module}/mod\.rs", f"crates/{module}/src/lib.rs", text)
    text = re.sub(rf"(?<![\w/])src/{module}/", f"crates/{module}/src/", text)
    named |= {m for m in re.findall(r"(?<!\w)rusty_(\w+)::", text) if m in others}
    return text, named


def by_hand(path: str, text: str) -> list[str]:
    """`path:line: why` for each line a text edit cannot settle."""
    checks = [("crate::{", "a `crate::{…}` group: split it per crate"),
              ("CARGO_MANIFEST_DIR", "names the crate's folder now: `rusty_core::project::checkout()`?"),
              ("crate::test_temp::lua_path", "the root's helper: build it from `rusty_test_support::temp::dir()`")]
    out = [f"{path}:{n}: {why}" for n, line in enumerate(text.splitlines(), 1)
           for needle, why in checks if needle in line]
    for n, line in enumerate(text.splitlines(), 1):
        for f in re.findall(r'feature\s*=\s*"([\w-]+)"', line):
            if f not in FEATURES:
                out.append(f"{path}:{n}: feature `{f}` is none of {'/'.join(FEATURES)}: declare and forward it")
    return out


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


def externs(code: str, deps: list[str]) -> list[str]:
    """The external crates `code` names as `name::` (`-` read as `_`), sorted."""
    return sorted(d for d in deps if re.search(rf"(?<![\w:]){d.replace('-', '_')}::", code))


class Plan:
    """The crate's manifest, read from what its code names and how the root declares each."""

    def __init__(self, root: Path, module: str, hand: list[str]) -> None:
        self.cargo = tomllib.loads((root / "Cargo.toml").read_text())
        self.root, self.module, self.hand = root, module, hand
        self.deps: dict[str, list[str]] = {}  # table header → `name = {…}` lines
        self.features: dict[str, set[str]] = {f: set() for f in FEATURES}

    def gate(self, name: str) -> str | None:
        """The one feature of the root that turns the optional `name` on; `None` if it is not optional.
        A sibling the root's `src/lib.rs` gates (`#[cfg(feature = "dev")] pub mod dev;`, moved
        or not) is optional behind that feature."""
        if name.startswith("rusty-"):
            m = name[len("rusty-"):]
            lib = (self.root / "src" / "lib.rs").read_text()
            gated = re.search(rf'#\[cfg\(feature = "([\w-]+)"\)\]\npub (?:mod {m}|use rusty_{m} as {m});', lib)
            if gated and gated.group(1) in FEATURES:
                return gated.group(1)
        specs = [self.cargo.get("dependencies", {}).get(name)]
        specs += [t.get("dependencies", {}).get(name) for t in self.cargo.get("target", {}).values()]
        if not any(isinstance(s, dict) and s.get("optional") for s in specs):
            return None
        on = [f for f, items in self.cargo.get("features", {}).items() if f"dep:{name}" in items]
        if len(on) == 1 and on[0] in FEATURES:
            return on[0]
        self.hand.append(f"crates/{self.module}/Cargo.toml: `{name}` is optional in the root, turned on by "
                         f"{on or 'no feature'}: declare its feature by hand")
        return "?"

    def table(self, name: str) -> str:
        """The root's table for `name`: a `[target.…]` one when only there, else `[dependencies]`."""
        if name in self.cargo.get("dependencies", {}):
            return "dependencies"
        for cfg, t in self.cargo.get("target", {}).items():
            if name in t.get("dependencies", {}):
                return f"target.'{cfg}'.dependencies"
        return "dependencies"

    def add(self, name: str, table: str | None = None) -> None:
        table = table or self.table(name)
        gate = self.gate(name) if table != "dev-dependencies" else None
        spec = "{ workspace = true, optional = true }" if gate else "{ workspace = true }"
        self.deps.setdefault(table, []).append(f"{name} = {spec}")
        if gate in FEATURES:
            self.features[gate].add(f"dep:{name}")

    def sibling(self, s: str) -> None:
        name = f"rusty-{s}"
        self.add(name, "dependencies")
        crate = self.root / layout.CRATES / s / "Cargo.toml"
        theirs = tomllib.loads(crate.read_text()).get("features", {}) if crate.is_file() else {}
        optional = any(name in d and "optional" in d for d in self.deps["dependencies"])
        for f in FEATURES:
            if f in theirs:
                self.features[f].add(f"{name}?/{f}" if optional else f"{name}/{f}")

    def manifest(self, used: set[str]) -> str:
        out = [f'[package]\nname = "rusty-{self.module}"',
               *(f"{k}.workspace = true" for k in ("version", "edition", "publish"))]
        declared = {f: sorted(v) for f, v in self.features.items() if v or f in used}
        if declared:
            rows = (f"{f} = [{', '.join(map(quote, v))}]" for f, v in declared.items())
            out.append("\n[features]\n" + "\n".join(rows))
        out.append("\n[lints]\nworkspace = true")
        self.deps.setdefault("dev-dependencies", []).insert(0, "rusty-test-support = { workspace = true }")
        order = ["dependencies", *sorted(t for t in self.deps if t.startswith("target.")), "dev-dependencies"]
        out += [f"\n[{t}]\n" + "\n".join(self.deps[t]) for t in order if self.deps.get(t)]
        return "\n".join(out) + "\n"


def root_gate(lib: str, module: str, hand: list[str]) -> str | None:
    """The feature gating `pub mod <m>;` in `src/lib.rs`, if any."""
    m = re.search(rf"(#\[cfg\(([^\n]*)\)\]\n)?pub mod {module};\n", lib)
    if not m or not m.group(1):
        return None
    one = re.fullmatch(r'feature\s*=\s*"([\w-]+)"', m.group(2).strip())
    if one and one.group(1) in FEATURES:
        return one.group(1)
    hand.append(f"src/lib.rs: `pub mod {module};` is gated by `{m.group(2)}`: gate the re-export and the root's "
                "dependency on the crate by hand")
    return None


def wire_root(root: Path, module: str, features: list[str], hand: list[str]) -> None:
    """The root manifest and `src/lib.rs` learn the crate."""
    lib = (root / "src" / "lib.rs").read_text()
    gate = root_gate(lib, module, hand)
    cargo = (root / "Cargo.toml").read_text()
    cargo = cargo.replace('rusty-test-support = { path = "crates/test-support" }',
                          f'rusty-{module} = {{ path = "crates/{module}" }}\n'
                          'rusty-test-support = { path = "crates/test-support" }', 1)
    spec = "{ workspace = true, optional = true }" if gate else "{ workspace = true }"
    cargo = re.sub(r"^(\[dependencies\]\n(?:#.*\n)*)", rf"\1rusty-{module} = {spec}\n", cargo, count=1, flags=re.M)
    for f in sorted({*features, *([gate] if gate else [])}):
        weak = "?" if gate and f != gate else ""
        adds = ([f"dep:rusty-{module}"] if f == gate else []) + ([f"rusty-{module}{weak}/{f}"] if f in features else [])
        cargo = re.sub(rf"^{f} = \[", f"{f} = [{', '.join(map(quote, adds))}, ", cargo, count=1, flags=re.M)
    (root / "Cargo.toml").write_text(cargo)
    lib = re.sub(rf"(#\[cfg\([^\n]*\)\]\n)?pub mod {module};\n", "", lib, count=1)
    last = list(re.finditer(r"^pub use rusty_\w+ as \w+;\n", lib, re.M))[-1]
    use = (f'#[cfg(feature = "{gate}")]\n' if gate else "") + f"pub use rusty_{module} as {module};\n"
    lib = lib[:last.end()] + use + lib[last.end():]
    (root / "src" / "lib.rs").write_text(lib)


def outside(root: Path, module: str) -> list[str]:
    """Each tracked file outside the crate still naming `src/<m>/`, with its lines."""
    done = subprocess.run(["git", "-C", str(root), "grep", "-n", "-F", f"src/{module}/", "--", ".",
                           f":(exclude)crates/{module}/**"], capture_output=True, text=True, check=False)
    hits: dict[str, list[str]] = {}
    for row in done.stdout.splitlines():
        path, line, _ = row.split(":", 2)
        hits.setdefault(path, []).append(line)
    return [f"{p}:{','.join(ls)}: names `src/{module}/`: rewrite a file path to `crates/{module}/src/`; "
            "a module path the lint tools read stays" for p, ls in sorted(hits.items())]


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
    siblings, live_code, test_code, hand = set(), [], [], []
    for path in sorted((crate / "src").rglob("*")):
        if path.suffix not in (".rs", ".md"):
            continue
        text, named = rewrite(path.read_text(), module, others)
        path.write_text(text)
        siblings |= named
        rel = path.relative_to(crate / "src")
        if is_test_file(rel):
            test_code.append(text)
        else:
            live_code.append(live(text))
            test_code.append(text)
        hand += by_hand(str(path.relative_to(root)), text)
    code = "\n".join(test_code)
    used = {f for f in FEATURES if f'feature = "{f}"' in code}
    deps = workspace_deps((root / "Cargo.toml").read_text())
    normal = externs("\n".join(live_code), deps)
    plan = Plan(root, module, hand)
    for s in sorted(siblings):
        plan.sibling(s)
    for d in normal:
        plan.add(d)
    for d in sorted(set(externs(code, deps)) - set(normal)):
        plan.add(d, "dev-dependencies")
    (crate / "Cargo.toml").write_text(plan.manifest(used))
    declared = [f for f in FEATURES if plan.features[f] or f in used]
    wire_root(root, module, declared, hand)
    return hand + outside(root, module)


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
