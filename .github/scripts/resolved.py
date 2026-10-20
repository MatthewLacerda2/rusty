#!/usr/bin/env python3
"""`resolved.py` — the commit hook's formatting check, over a whole branch (#1096).

The commit hook (`make pre-commit`) checks each commit as it is made, but two
paths reach CI without it:

- **`git rebase --continue` runs no pre-commit hook.** A conflict resolved by
  hand mid-rebase is committed unchecked, and that is exactly when hand-edits
  happen (a hand-back resolved under `make queue ARGS=--no-check`).
- **`tools/lint` is its own crate.** `cargo fmt --all` from the root never
  reaches it, so before #1096 nothing local formatted it at all.

In the 2026-10-20 batch #1023 and #1052 went red on rustfmt after a keep-both
resolution left `tools/lint/src/rules/mod.rs`'s `mod` lines out of order, a CI
round-trip each. This runs from the `pre-push` hook (`.githooks/pre-push`) and as
`make resolved`: it takes the files a branch changes against `origin/main` and
runs rustfmt's check on each crate they touch. No build, so about a second.

It reads the working tree, as rustfmt does, so it checks a pushed tip only when
that tip is `HEAD`; pushing some other branch is let through. The size gate stays
in the commit hook and CI: in the merge queue's fresh worktree it would first
compile `tools/lint`, which is not a second. Exit 1 names the command that fixes it.
"""

from __future__ import annotations

import subprocess
import sys

ROOT_FMT = ["cargo", "fmt", "--all", "--check"]
LINT_FMT = ["cargo", "fmt", "--manifest-path", "tools/lint/Cargo.toml", "--check"]
ZERO = "0" * 40


def git(*args: str, cwd: str | None = None) -> str:
    """A git command's stdout, or "" when it fails."""
    done = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else ""


def checks(paths: list[str]) -> list[list[str]]:
    """The rustfmt checks the changed `paths` call for: one per crate they touch."""
    lint = [p for p in paths if p.startswith("tools/lint/")]
    engine = [p for p in paths if p.endswith(".rs") and not p.startswith("tools/lint/")]
    wanted = []
    if engine:
        wanted.append(ROOT_FMT)
    if any(p.endswith((".rs", "Cargo.toml")) for p in lint):
        wanted.append(LINT_FMT)
    return wanted


def pushed_tips(hook_input: str) -> list[str]:
    """The local commits a `pre-push` hook's stdin says are being pushed (deletes dropped)."""
    tips = []
    for line in hook_input.splitlines():
        fields = line.split()
        if len(fields) == 4 and fields[1] != ZERO:
            tips.append(fields[1])
    return tips


def changed(tip: str, cwd: str | None = None) -> list[str] | None:
    """Files `tip` changes against its merge base with `origin/main`; None without one."""
    base = git("merge-base", tip, "origin/main", cwd=cwd)
    if not base:
        return None
    names = git("diff", "--name-only", "--diff-filter=ACMR", base, tip, cwd=cwd)
    return names.splitlines()


def run(tips: list[str], cwd: str | None = None, quiet: bool = False) -> int:
    """Check every tip that is `HEAD`; 1 when a check fails. `quiet` drops rustfmt's diff."""
    head = git("rev-parse", "HEAD", cwd=cwd)
    paths: set[str] = set()
    for tip in tips:
        if git("rev-parse", tip, cwd=cwd) != head:
            continue
        files = changed(tip, cwd)
        if files is None:
            print("resolved: no merge base with origin/main; nothing checked", file=sys.stderr)
            continue
        paths.update(files)
    failed = [
        c for c in checks(sorted(paths))
        if subprocess.run(c, cwd=cwd, capture_output=quiet).returncode != 0
    ]
    if failed and not quiet:
        for command in failed:
            fix = " ".join(a for a in command if a != "--check")
            print(f"resolved: `{' '.join(command)}` failed; run `{fix}` and commit", file=sys.stderr)
        print("resolved: (`git push --no-verify` skips this; CI does not)", file=sys.stderr)
    return 1 if failed else 0


def main() -> int:
    if sys.argv[1:] == ["--hook"]:
        return run(pushed_tips(sys.stdin.read()))
    return run(["HEAD"])


if __name__ == "__main__":
    sys.exit(main())
