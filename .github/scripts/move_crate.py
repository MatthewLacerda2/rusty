#!/usr/bin/env python3
"""`make move-crate MODULE=<m>`: the mechanical half of moving a module into its crate.

The engine is split one crate per module, over many pull requests
(`crates/CLAUDE.md` is the recipe). Most of a move is the same edits every time,
so this makes them, and lists what it cannot decide:

- `git mv src/<m> crates/<m>/src`, its `mod.rs` becoming `lib.rs`;
- in the crate's files: `crate::<dep>::` → `rusty_<dep>::` for each module its
  row of the layer table (`tools/lint/src/layers/table.rs`) lists as a dep, then `crate::<m>::` → `crate::` (in that order, so a
  submodule named like a module, `crate::render::scene::`, stays the crate's
  own), the shared test helper (`crate::test_temp::dir()` →
  `rusty_test_support::temp::dir()`), each `src/<m>/` path it names →
  `crates/<m>/src/`, `lib.rs`'s header naming its directory (`//! src/<m> — …`,
  as the `mod.rs` it was may) → `crates/<m>/src/lib.rs` (`lib_header`), and each
  relative `include_str!` / `include_bytes!` path re-aimed from the file's new
  folder (in any tracked file whose include names a file of the crate, too);
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
  feature forwards, each external crate the crate took that nothing left in the
  root names dropped from its `[dependencies]`, and `src/lib.rs`'s `pub mod <m>;` → `pub use rusty_<m> as <m>;`.
  A `pub mod` gated by `#[cfg(feature = "f")]` stays gated: the root depends on
  the crate optionally, its feature `f` turns it on (`"dep:rusty-<m>"`), and the
  re-export carries the same `cfg`.

What it prints as **by hand** is what a text edit cannot settle: a
`crate::<x>::` naming a module the row does not list (a unit test reaching up the
layering, which moves to `tests/` or to `<x>`'s own tests: no crate may depend on
a module above it), a `crate::{…}`
group (its entries may name several modules), `CARGO_MANIFEST_DIR` (it now names
the crate's folder), a string path from the checkout's root (`"tests/fixtures/…"`,
read from the working directory, which is the crate's folder under its tests), `crate::test_temp::lua_path`, an optional dependency no one
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
import os
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


TABLE = Path("tools/lint/src/layers/table.rs")


def row_deps(root: Path, module: str) -> set[str]:
    """The modules `module`'s row of the layer table may import."""
    row = re.search(rf'(?:sim|platform)\(\s*"{module}",\s*&\[([^\]]*)\]', (root / TABLE).read_text())
    if not row:
        sys.exit(f"move-crate: {TABLE} has no row for `{module}`")
    return set(re.findall(r'"(\w+)"', row.group(1)))


def rewrite(text: str, module: str, deps: set[str]) -> tuple[str, set[str]]:
    """`text` with its module paths rewritten for the crate, and the sibling modules it names.
    Only the row's `deps` are rewritten: a module above it stays `crate::<x>::`, for `by_hand`."""
    named = set()

    def sibling(m: re.Match) -> str:
        if m.group(1) not in deps:
            return m.group(0)
        named.add(m.group(1))
        return f"rusty_{m.group(1)}::"

    # Siblings first: `crate::<m>::scene::` must not read as the sibling `scene`.
    text = re.sub(r"(?<![\w$])crate::(\w+)::", sibling, text)
    text = text.replace(f"crate::{module}::", "crate::")
    text = text.replace("crate::test_temp::dir()", "rusty_test_support::temp::dir()")
    text = re.sub(rf"(?<![\w/])src/{module}/mod\.rs", f"crates/{module}/src/lib.rs", text)
    text = re.sub(rf"(?<![\w/])src/{module}/", f"crates/{module}/src/", text)
    named |= {m for m in re.findall(r"(?<!\w)rusty_(\w+)::", text) if m in deps}
    return text, named


def lib_header(text: str, module: str) -> str:
    """`text` with a leading `//! src/<m>` or `//! crates/<m>/src/` header naming `lib.rs`
    instead. A `mod.rs` header may name its directory, which `lib.rs`'s may not: the
    rewrite once left `shadergen`'s `//! src/shadergen — …` a stale path, which
    `self-path` now flags (#2140)."""
    lines = text.split("\n")
    for n, line in enumerate(lines):
        if line.startswith("//! "):
            word = rf"(?<=^//! )(?:src/{module}/?|crates/{module}/src/?)(?=[\s:,—]|$)"
            lines[n] = re.sub(word, f"crates/{module}/src/lib.rs", line, count=1)
            break
    return "\n".join(lines)


def reaching_up(path: str, text: str, above: set[str]) -> list[str]:
    """`path:line: why` for each `crate::<x>::` naming a module the row does not list. Read in
    the text as it was: once `crate::<m>::` is `crate::`, the crate's own submodule named
    like a module above it (`crate::<m>::ui::`) would read as that module, a false report PR #2149's review
    found."""
    return [f"{path}:{n}: names `{x}`, which the row does not list: a test reaching up moves to "
            f"`tests/` (or `{x}`'s own tests); live code is a layering violation"
            for n, line in enumerate(text.splitlines(), 1)
            for x in sorted(set(re.findall(r"(?<![\w$])crate::(\w+)(?!\w)", line)) & above)]


#: A string literal naming a path from the checkout's root (`"tests/fixtures/…"`), the crate's
#: own `"crates/<m>/src/…"` (a `"src/<m>/…"` the rewrite re-aimed) among them: PR #2149's review
#: found the rewrite re-aiming such a literal silently, though the crate's tests read it from the
#: crate's folder, where it names nothing.
CWD_PATH = re.compile(r'(?<![\w/])"(?:tests|engine|games|docs|src|crates)/')


def by_hand(path: str, text: str, before: str) -> list[str]:
    """`path:line: why` for each line a text edit cannot settle. A `crate::{…}` group is read in
    `before`, the text as it was: a `crate::<m>::{…}` group was the module's own, and stays so."""
    groups = [f"{path}:{n}: a `crate::{{…}}` group: split it per crate"
              for n, line in enumerate(before.splitlines(), 1) if "crate::{" in line]
    checks = [("CARGO_MANIFEST_DIR", "names the crate's folder now: `rusty_core::project::checkout()`?"),
              ("crate::test_temp::lua_path", "the root's helper: build it from `rusty_test_support::temp::dir()`")]
    out = groups + [f"{path}:{n}: {why}" for n, line in enumerate(text.splitlines(), 1)
                    for needle, why in checks if needle in line]
    for n, line in enumerate(text.splitlines(), 1):
        if CWD_PATH.search(line) and "checkout()" not in line and not line.lstrip().startswith("//"):
            out.append(f"{path}:{n}: a path from the checkout's root read from the working directory, the "
                       "crate's folder under its tests: `rusty_core::project::checkout().join(…)`?")
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

    def declared(self, used: set[str]) -> dict[str, list[str]]:
        """The features the crate declares, each with what it turns on: one its own code names
        (`used`) or an optional dependency needs. A crate with neither declares none, even beside
        a sibling that has it: the root turns each member's feature on directly, so only a crate
        that declares one forwards it to its siblings (for its own `-p` builds)."""
        return {f: sorted(v) for f, v in self.features.items() if f in used or any(x.startswith("dep:") for x in v)}

    def manifest(self, used: set[str]) -> str:
        out = [f'[package]\nname = "rusty-{self.module}"',
               *(f"{k}.workspace = true" for k in ("version", "edition", "publish"))]
        declared = self.declared(used)
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


def leave_root(root: Path, module: str, externs_: list[str]) -> list[str]:
    """Drop from the root's `[dependencies]` each external crate the moved crate takes
    that no `.rs` file left outside `crates/` names; the crates dropped. Only inside that
    table: PR #2149's review found the old match running on past it, into a later table's
    line of the same name (`[dev-dependencies]`, a `[target.…]` one)."""
    left = subprocess.run(["git", "-C", str(root), "ls-files", "--", "*.rs", f":(exclude){layout.CRATES}/**"],
                          capture_output=True, text=True, check=False).stdout.split()
    code = [(root / f).read_text() for f in left if (root / f).is_file()]
    cargo = (root / "Cargo.toml").read_text()
    start = cargo.index("\n[dependencies]\n") + 1
    end = cargo.find("\n[", start)
    end = len(cargo) if end < 0 else end + 1
    table = cargo[start:end]
    gone = [d for d in externs_ if not externs(code, [d])
            and re.search(rf"^{re.escape(d)} = \{{ workspace = true \}}\n", table, re.M)]
    for d in gone:
        table = re.sub(rf"^{re.escape(d)} = \{{ workspace = true \}}\n", "", table, count=1, flags=re.M)
    cargo = cargo[:start] + table + cargo[end:]
    (root / "Cargo.toml").write_text(cargo)
    return gone


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
    deps = row_deps(root, module)
    crate.mkdir(parents=True)
    git = ["git", "-C", str(root), "mv"]
    subprocess.run([*git, f"src/{module}", f"crates/{module}/src"], check=True)
    subprocess.run([*git, f"crates/{module}/src/mod.rs", f"crates/{module}/src/lib.rs"], check=True)
    siblings, live_code, test_code, hand = set(), [], [], []
    for path in sorted((crate / "src").rglob("*")):
        if path.suffix not in (".rs", ".md"):
            continue
        before = path.read_text()
        rel = path.relative_to(crate / "src")
        text, named = rewrite(before, module, deps & others)
        text = reaim(text, Path("src", module, rel), Path(layout.CRATES, module, "src", rel), module)
        if rel == Path("lib.rs"):
            text = lib_header(text, module)
        path.write_text(text)
        siblings |= named
        if is_test_file(rel):
            test_code.append(text)
        else:
            live_code.append(live(text))
            test_code.append(text)
        hand += by_hand(str(path.relative_to(root)), text, before)
        hand += reaching_up(str(path.relative_to(root)), before, others - deps)
    code = "\n".join(test_code)
    used = {f for f in FEATURES if f'feature = "{f}"' in code}
    registry = workspace_deps((root / "Cargo.toml").read_text())
    normal = externs(live_code, registry)
    plan = Plan(root, module, hand)
    for s in sorted(siblings):
        plan.sibling(s)
    for d in normal:
        plan.add(d)
    for d in sorted(set(externs(test_code, registry)) - set(normal)):
        plan.add(d, "dev-dependencies")
    (crate / "Cargo.toml").write_text(plan.manifest(used))
    declared = list(plan.declared(used))
    wire_root(root, module, declared, hand)
    reaim_outside(root, module)
    gone = leave_root(root, module, normal)
    if gone:
        hand.append(f"Cargo.toml: {', '.join(gone)} left the root's [dependencies] (only the crate names them now)")
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
    print("next: `cargo fmt --all` (the rewritten imports re-sort and re-wrap), "
          "`cargo check --workspace --all-targets --features dev` and `--no-default-features`, "
          "then crates/CLAUDE.md, *What the compiler will name*.")


if __name__ == "__main__":
    main()
