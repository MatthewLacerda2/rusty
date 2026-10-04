#!/usr/bin/env python3
"""Turn the scope someone names into a mutation run over exactly that (#750).

`make mutants-remote SCOPE=...` hands one string to `mutants-on-request.yml`,
and this is what the workflow asks to make sense of it. Two spellings:

- **`diff`** — the Rust this branch changed against `origin/main`: "would
  anything notice if what I just wrote were wrong?"
- **anything else** — one or more git pathspec globs, space-separated
  (`src/physics/raycast.rs`, `'src/navigation/**'`). `**` crosses
  directories and `*` does not, git's own `:(glob)` rules.

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

    mutants-scope.py resolve 'src/navigation/**' --out plan.json --diff scope.diff
    cargo mutants --list --json --in-diff scope.diff > planned.json
    mutants-scope.py check plan.json planned.json > scope.txt
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
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
    """The tracked Rust files the globs name, in git's glob dialect."""
    listed = git("ls-files", "-z", "--", *(f":(glob){g}" for g in globs))
    return sorted(p for p in listed.split("\0") if p.endswith(".rs"))


def whole(files: list[str]) -> str:
    """A diff that adds each file entire, so `--in-diff` keeps all of it.

    `git diff --no-index` exits 1 when the sides differ, which against
    `/dev/null` they always do; that is the answer, not a failure.
    """
    return "".join(git("diff", "--no-index", "--", "/dev/null", f, ok=(0, 1)) for f in files)


def branch_diff(base: str) -> tuple[str, list[str]]:
    """What this branch changed in Rust since it left `base`."""
    if not git("rev-parse", "--verify", "--quiet", f"{base}^{{commit}}", ok=(0, 1)).strip():
        sys.exit(f"mutants-scope: {base} is not in this checkout; a diff against it would be empty")
    fork = git("merge-base", base, "HEAD").strip()
    names = git("diff", "--name-only", fork, "HEAD", "--", "*.rs").split()
    return git("diff", fork, "HEAD", "--", "*.rs"), sorted(names)


def resolve(scope: str, base: str) -> tuple[dict, str]:
    """The plan this scope asks for, and the diff `--in-diff` reads."""
    which = kind(scope)
    if which == "diff":
        diff, files = branch_diff(base)
    else:
        files = matching(scope.split())
        if not files:
            sys.exit(f"mutants-scope: no tracked Rust file matches {scope!r}")
        diff = whole(files)
    return {"kind": which, "scope": scope, "base": base, "files": files}, diff


def check(plan: dict, planned: list[dict]) -> tuple[str, bool]:
    """The report's scope line, and whether the run should go ahead at all.

    Nothing planned is an answer for a `diff` (a branch that touched only
    tests) and an error for named paths.
    """
    count = len(planned)
    noun = "mutant" if count == 1 else "mutants"
    files = plan["files"]
    if plan["kind"] == "diff":
        subject = f"the Rust this branch changed against `{plan['base']}` ({len(files)} files)"
    else:
        shown = ", ".join(f"`{f}`" for f in files[:5])
        more = f" and {len(files) - 5} more" if len(files) > 5 else ""
        subject = f"{shown}{more}, matched by `{plan['scope']}`"
    line = f"Scoped on request to {subject}: {count} {noun} planned."
    named = set(files)
    stray = sorted({m.get("file", "?") for m in planned if m.get("file") not in named})
    if stray:
        line += (
            f" Some are from outside that scope ({', '.join(f'`{f}`' for f in stray[:5])}"
            f"{' and more' if len(stray) > 5 else ''}) and are reported anyway;"
            " read those rows as not yours."
        )
    if count == 0 and plan["kind"] != "diff":
        return line + " None of it has anything cargo-mutants can mutate.", False
    return line, True


def main() -> None:
    parser = argparse.ArgumentParser(prog="mutants-scope.py", description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    res = sub.add_parser("resolve", help="turn a scope into a plan and a diff")
    res.add_argument("scope", help="`diff`, or space-separated path globs")
    res.add_argument("--base", default=BASE, help="what a `diff` scope compares against")
    res.add_argument("--out", type=Path, required=True, help="where the plan JSON goes")
    res.add_argument("--diff", type=Path, required=True, help="where the scope's diff goes")
    chk = sub.add_parser("check", help="the scope line, from what --list planned")
    chk.add_argument("plan", type=Path, help="the JSON `resolve` wrote")
    chk.add_argument("planned", type=Path, help="`cargo mutants --list --json` output")
    args = parser.parse_args()

    if args.command == "resolve":
        plan, diff = resolve(args.scope, args.base)
        args.out.write_text(json.dumps(plan))
        args.diff.write_text(diff)
        return
    # cargo-mutants prints nothing at all, not `[]`, when nothing is in scope.
    listed = args.planned.read_text().strip()
    line, go = check(json.loads(args.plan.read_text()), json.loads(listed) if listed else [])
    print(line)
    if not go:
        sys.exit(1)


if __name__ == "__main__":
    main()
