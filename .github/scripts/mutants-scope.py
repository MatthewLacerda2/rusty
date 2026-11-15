#!/usr/bin/env python3
"""Turn the scope someone names into a mutation run over exactly that (#750).

`make mutants-remote SCOPE=...` hands one string to `mutants-on-request.yml`,
and this is what the workflow asks to make sense of it. Two spellings:

- **`diff`** — the Rust this branch changed against `origin/main`: "would
  anything notice if what I just wrote were wrong?"
- **anything else** — one or more git pathspec globs, space-separated
  (`crates/physics/src/query/through.rs`, `'crates/navigation/src/**'`). `**` crosses
  directories and `*` does not, git's own `:(glob)` rules.

A third form is the merge queue's (#1934): **`diff` with `--commit SHA`**, the
Rust one squash commit changed against its parent. The queue dispatches a
merged pull request's run on `main` with the commit checked out, so the
branch is gone and `diff` against `origin/main` would be empty; the commit's
own diff is the branch's diff as it landed, so the same mutants are asked
about as a `diff` run on the branch would have been.

Both become one cargo-mutants argument, `--in-diff`: it keeps only mutants
whose span touches a changed line. A path scope becomes a diff that adds the
named files whole, from `/dev/null`, which touches every line of them and of
nothing else. scorsese found the hard way that `--file` and `--re` do not
narrow for real (`--file` is unioned with a config's `examine_globs`;
struct-field deletions ignore `--re`), so the one spelling that does is used
for both.

The narrowing is still checked rather than trusted: `check` reads what
`cargo mutants --list --json` planned against the files the scope named and
says in the report's scope line how many planned mutants came from anywhere
else. A named path with nothing to mutate is refused, red: reporting zero
survivors for it would be a clean bill of health over an absence.

**One plan per cargo workspace** (#2201). cargo-mutants mutates the packages
of the workspace it runs in, and `tools/lint` is a crate with its own
`[workspace]`, so a root-workspace run planned none of its files: three asks
for lint rules in one day got "0 mutants planned". The scope's files are split
by the workspace each lies in ([`workspace`]: the nearest directory above it
whose `Cargo.toml` opens its own `[workspace]`, found, not listed), and each
part gets its own diff, with paths relative to its root, for a run in that
directory. A scope that mixes the engine and a standalone crate (a new lint
rule and the code it fixes) is two plans in one report, never a refusal. A
part with nothing to run in it (a cargo-fuzz crate: nightly-only, no tests of
its own) is named as such, not failed.

    mutants-scope.py resolve 'crates/navigation/src/**' --out plan.json
    mutants-scope.py parts plan.json      # `slug root diff` per part to list
    cargo mutants --list --json [--dir ROOT] --in-diff scope-SLUG.diff > planned-SLUG.json
    mutants-scope.py check plan.json > scope.txt
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from functools import cache
from pathlib import Path

#: What a `diff` scope is measured against: the branch everything merges into.
BASE = "origin/main"


def git(*args: str, ok: tuple[int, ...] = (0,)) -> str:
    """`git`'s stdout, dying with git's own words on anything else."""
    done = subprocess.run(["git", *args], capture_output=True, text=True, check=False)
    if done.returncode not in ok:
        sys.exit(f"mutants-scope: git {' '.join(args)}: {done.stderr.strip()}")
    return done.stdout


def kind(scope: str) -> str:
    """Which spelling `scope` is. Empty is an error, never a default."""
    words = scope.split()
    if not words:
        sys.exit("mutants-scope: the scope is empty -- name `diff` or path globs")
    return "diff" if words == ["diff"] else "paths"


def matching(globs: list[str]) -> list[str]:
    """The tracked Rust files the globs name, in git's glob dialect. One glob
    that matches nothing is refused, not only all of them: a module moved to
    `crates/<m>/src/` leaves its old `src/<m>/**` glob naming nothing, and the
    other globs would hide it."""
    found: set[str] = set()
    for g in globs:
        listed = git("ls-files", "-z", "--", f":(glob){g}")
        rust = {p for p in listed.split("\0") if p.endswith(".rs")}
        if not rust:
            sys.exit(f"mutants-scope: no tracked Rust file matches {g!r} -- moved to crates/? (crates/CLAUDE.md)")
        found |= rust
    return sorted(found)


#: The engine's own workspace, the repository root.
ROOT = "."


@cache
def manifest(directory: str) -> dict | None:
    """`directory/Cargo.toml`, parsed, or `None` when there is none."""
    path = Path(directory, "Cargo.toml")
    if not path.is_file():
        return None
    try:
        return tomllib.loads(path.read_text())
    except tomllib.TOMLDecodeError as err:
        sys.exit(f"mutants-scope: {path} is not TOML ({err}); cannot tell which workspace it opens")


def workspace(path: str) -> str:
    """The root of the cargo workspace `path` is in: the nearest directory above
    it whose `Cargo.toml` opens its own `[workspace]`, else the repository's."""
    here = Path(path).parent
    while here != Path(ROOT):
        if "workspace" in (manifest(here.as_posix()) or {}):
            return here.as_posix()
        here = here.parent
    return ROOT


def slug(root: str) -> str:
    """The name a part's files go by: `root` for the engine, else its path."""
    return "root" if root == ROOT else root.replace("/", "-")


def idle(root: str) -> str | None:
    """Why nothing can run in a standalone workspace, or `None` when it can.
    A cargo-fuzz crate needs nightly and has no tests to catch a mutant."""
    if "cargo-fuzz" in (manifest(root) or {}).get("package", {}).get("metadata", {}):
        return "a cargo-fuzz crate, nightly-only and with no tests of its own to catch a mutant"
    return None


def relative(root: str, path: str) -> str:
    """`path` as cargo-mutants names it in a run inside `root`."""
    return path if root == ROOT else Path(path).relative_to(root).as_posix()


def whole(root: str, files: list[str]) -> str:
    """A diff that adds each file entire, so `--in-diff` keeps all of it; paths
    are relative to `root`, the directory its run is in.

    `git diff --no-index` exits 1 when the sides differ, which against
    `/dev/null` they always do; that is the answer, not a failure.
    """
    return "".join(git("-C", root, "diff", "--no-index", "--", "/dev/null", relative(root, f), ok=(0, 1))
                   for f in files)


def fork_point(base: str) -> str:
    """Where this branch left `base`; a `base` the checkout lacks is refused."""
    if not git("rev-parse", "--verify", "--quiet", f"{base}^{{commit}}", ok=(0, 1)).strip():
        sys.exit(f"mutants-scope: {base} is not in this checkout; a diff against it would be empty")
    return git("merge-base", base, "HEAD").strip()


def changed(fork: str, root: str, files: list[str]) -> str:
    """What this branch changed in `files` since `fork`, relative to `root`."""
    rel = [] if root == ROOT else [f"--relative={root}/"]
    return git("diff", *rel, fork, "HEAD", "--", *files)


def resolve(scope: str, base: str, commit: str | None = None) -> tuple[dict, dict[str, str]]:
    """The plan this scope asks for, and each part's diff by its file name.
    With `commit`, a `diff` is that commit's against its parent (#1934)."""
    which, fork = kind(scope), ""
    if which == "diff":
        if commit:
            base = f"{commit}^"
        fork = fork_point(base)
        files = sorted(git("diff", "--name-only", fork, "HEAD", "--", "*.rs").split())
    else:
        files = matching(scope.split())
        if not files:
            sys.exit(f"mutants-scope: no tracked Rust file matches {scope!r}")
    roots: dict[str, list[str]] = {}
    for f in files:
        roots.setdefault(workspace(f), []).append(f)
    parts, diffs = [], {}
    for root in sorted(roots, key=lambda r: (r != ROOT, r)):
        mine, name = roots[root], f"scope-{slug(root)}.diff"
        part = {"root": root, "slug": slug(root), "files": mine, "diff": name}
        if root != ROOT and (why := idle(root)):
            part["idle"] = why
        else:
            diffs[name] = changed(fork, root, mine) if which == "diff" else whole(root, mine)
        parts.append(part)
    plan = {"kind": which, "scope": scope, "base": base, "files": files, "parts": parts}
    if commit:
        plan["commit"] = commit
    return plan, diffs


def subject(plan: dict) -> str:
    """What the scope named, for the report's first line."""
    files = plan["files"]
    if plan["kind"] == "diff" and plan.get("commit"):
        return f"the Rust commit `{plan['commit'][:9]}` changed ({len(files)} files)"
    if plan["kind"] == "diff":
        return f"the Rust this branch changed against `{plan['base']}` ({len(files)} files)"
    shown = ", ".join(f"`{f}`" for f in files[:5])
    more = f" and {len(files) - 5} more" if len(files) > 5 else ""
    return f"{shown}{more}, matched by `{plan['scope']}`"


def counted(part: dict, planned: list[dict], paths: bool) -> str:
    """One part's count, the mutants from outside it, and a named part's absence."""
    count = len(planned)
    line = f"{count} {'mutant' if count == 1 else 'mutants'} planned."
    named = {relative(part["root"], f) for f in part["files"]}
    stray = sorted({m.get("file", "?") for m in planned if m.get("file") not in named})
    if stray:
        line += (
            f" Some are from outside that scope ({', '.join(f'`{f}`' for f in stray[:5])}"
            f"{' and more' if len(stray) > 5 else ''}) and are reported anyway;"
            " read those rows as not yours."
        )
    if count == 0 and paths:
        line += " None of it has anything cargo-mutants can mutate."
    return line


def check(plan: dict, planned: dict[str, list[dict]]) -> tuple[str, bool]:
    """The report's scope lines, and whether the run should go ahead at all.
    `planned` is each runnable part's `--list` by slug; each part's count is
    written into its entry (`planned`), which the report reads.

    Nothing planned is an answer for a `diff` (a branch that touched only
    tests) and an error for named paths, unless every part named is one
    nothing can run in, which is said rather than failed.
    """
    paths = plan["kind"] != "diff"
    parts = plan["parts"]
    if not parts:
        return f"Scoped on request to {subject(plan)}: 0 mutants planned.", True
    lines = []
    for part in parts:
        if "idle" in part:
            part["planned"] = 0
            lines.append(f"`{part['root']}/` is its own workspace with nothing to run here: {part['idle']}.")
            continue
        listed = planned.get(part["slug"], [])
        part["planned"] = len(listed)
        where = "" if part["root"] == ROOT else f"in `{part['root']}/`, its own workspace, "
        lines.append(where + counted(part, listed, paths))
    total = sum(p["planned"] for p in parts)
    runnable = any("idle" not in p for p in parts)
    go = not (paths and runnable and total == 0)
    if len(parts) == 1:
        return f"Scoped on request to {subject(plan)}: {lines[0]}", go
    return "\n".join([f"Scoped on request to {subject(plan)}, in {len(parts)} plans:", "",
                      *(f"- {line}" for line in lines)]), go


def listed(path: Path) -> list[dict]:
    """A `cargo mutants --list --json` file, which is empty, not `[]`, when
    nothing is in scope; a missing file planned nothing."""
    text = path.read_text().strip() if path.is_file() else ""
    return json.loads(text) if text else []


def main() -> None:
    parser = argparse.ArgumentParser(prog="mutants-scope.py", description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    res = sub.add_parser("resolve", help="turn a scope into a plan and one diff per workspace")
    res.add_argument("scope", help="`diff`, or space-separated path globs")
    res.add_argument("--base", default=BASE, help="what a `diff` scope compares against")
    res.add_argument("--commit", help="the checked-out commit a `diff` is that commit's own change (#1934)")
    res.add_argument("--out", type=Path, required=True, help="where the plan JSON goes; the diffs go beside it")
    globs = sub.add_parser("globs", help="refuse any glob that names no tracked Rust file")
    globs.add_argument("glob", nargs="+")
    each = sub.add_parser("parts", help="`slug root diff` for each part a run can list")
    each.add_argument("plan", type=Path, help="the JSON `resolve` wrote")
    each.add_argument("--standalone", action="store_true", help="only the parts outside the engine's workspace")
    each.add_argument("--planned", action="store_true", help="only the parts `check` counted a mutant in")
    chk = sub.add_parser("check", help="the scope lines, from each part's planned-SLUG.json beside the plan")
    chk.add_argument("plan", type=Path, help="the JSON `resolve` wrote; each part's count is written into it")
    args = parser.parse_args()

    if args.command == "globs":
        print(f"mutants-scope: {len(matching(args.glob))} files")
        return
    if args.command == "resolve":
        plan, diffs = resolve(args.scope, args.base, args.commit or None)
        args.out.write_text(json.dumps(plan))
        for name, diff in diffs.items():
            (args.out.parent / name).write_text(diff)
        return
    plan = json.loads(args.plan.read_text())
    if args.command == "parts":
        for part in plan["parts"]:
            if "idle" not in part and not (args.standalone and part["root"] == ROOT) \
                    and not (args.planned and not part.get("planned")):
                print(part["slug"], part["root"], part["diff"])
        return
    here = args.plan.parent
    line, go = check(plan, {p["slug"]: listed(here / f"planned-{p['slug']}.json") for p in plan["parts"]})
    args.plan.write_text(json.dumps(plan))
    print(line)
    if not go:
        sys.exit(1)


if __name__ == "__main__":
    main()
