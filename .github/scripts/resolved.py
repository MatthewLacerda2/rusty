#!/usr/bin/env python3
"""`resolved.py` — the commit hook's formatting check, over a whole branch (#1096).

The commit hook (`make pre-commit`) checks each commit as it is made, but two
paths reach CI without it:

- **A conflict resolved by hand can be committed unchecked.** `git rebase
  --continue` runs no pre-commit hook, and a merge commit concluded with
  `git commit --no-verify` skips it too; that is exactly when hand-edits
  happen (a hand-back resolved under `make queue ARGS=--no-check`). Since #1568 a branch is updated by merging `main` in: the
  merge commit is the pushed tip, `HEAD`, so this check reads the resolved tree
  whichever way it was committed.
- **`tools/lint` is its own crate.** `cargo fmt --all` from the root never
  reaches it, so before #1096 nothing local formatted it at all.

In the 2026-10-20 batch #1023 and #1052 went red on rustfmt after a keep-both
resolution left `tools/lint/src/rules/mod.rs`'s `mod` lines out of order, a CI
round-trip each. This runs from the `pre-push` hook (`.githooks/pre-push`) and as
`make resolved`: both crates' rustfmt checks, exactly the pair `lint.yml` runs.
No build: the same rustfmt the commit hook already pays per commit.

It runs both every time rather than only for the crates a branch touches: an
earlier cut diffed against `origin/main`'s merge base, and passed having checked
nothing when there was none (a shallow clone) and missed a `rustfmt.toml` or
edition change (#1101's independent review).

It reads the working tree, as rustfmt does, so the hook checks a pushed tip only
when that tip is `HEAD`; pushing some other branch is let through. The size gate
stays in the commit hook and CI: in the merge queue's fresh worktree it would first
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


def pushed_tips(hook_input: str) -> list[str]:
    """The local commits a `pre-push` hook's stdin says are being pushed (deletes dropped)."""
    tips = []
    for line in hook_input.splitlines():
        fields = line.split()
        if len(fields) == 4 and fields[1] != ZERO:
            tips.append(fields[1])
    return tips


def pushes_head(tips: list[str], cwd: str | None = None) -> bool:
    """Whether one of `tips` is `HEAD`, the only commit the working tree stands for."""
    head = git("rev-parse", "HEAD", cwd=cwd)
    return any(git("rev-parse", tip, cwd=cwd) == head for tip in tips)


def run(cwd: str | None = None, quiet: bool = False) -> int:
    """Both crates' rustfmt checks; 1 when one fails. `quiet` drops rustfmt's diff."""
    failed = [
        c for c in (ROOT_FMT, LINT_FMT)
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
        return run() if pushes_head(pushed_tips(sys.stdin.read())) else 0
    return run()


if __name__ == "__main__":
    sys.exit(main())
