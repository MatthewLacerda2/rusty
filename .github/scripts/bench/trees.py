"""`main` and a pull request's head, each in a detached worktree of its own (#2381).

A PR's head is fetched as `pull/<n>/head`, never as a named branch: a fork's PR
has none here, and a branch name can move during a run. Each tree lives under
the main checkout's `.claude/worktrees/` (CLAUDE.md: one worktree per branch,
never a shared `CARGO_TARGET_DIR`), so it builds into its own `target/` and is
kept warm for the next run, and `make reclaim` removes it once its commit is on
`main` or its PR is closed. A tree is re-pointed with a plain `checkout
--detach`, so one with local changes fails rather than losing them.
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

MAIN_TREE = "bench-main"


def pr_tree(pr: int) -> str:
    """The worktree folder a PR's head is checked out in."""
    return f"pr-{pr}-head"


def run(argv, cwd=None, env=None) -> str:
    """`argv`'s stdout; a failure raises with its stderr."""
    done = subprocess.run(argv, cwd=cwd, env=env, text=True, capture_output=True)
    if done.returncode != 0:
        raise RuntimeError(f"{' '.join(map(str, argv))}: {done.stderr.strip() or done.stdout.strip()}")
    return done.stdout


def main_checkout(git=run) -> Path:
    """The main checkout (the common git dir's parent), wherever this runs from."""
    common = Path(git(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"]).strip())
    return common.parent


def fetch(pr: int | None, git=run) -> tuple[str, str | None]:
    """The commits of `origin/main` and of PR `pr`'s head, fetched now."""
    git(["git", "fetch", "--quiet", "origin", "main"])
    main = git(["git", "rev-parse", "origin/main"]).strip()
    if pr is None:
        return main, None
    git(["git", "fetch", "--quiet", "origin", f"pull/{pr}/head"])
    return main, git(["git", "rev-parse", "FETCH_HEAD"]).strip()


def checkout(root: Path, name: str, sha: str, git=run) -> Path:
    """`sha` checked out detached in `<root>/.claude/worktrees/<name>`."""
    tree = root / ".claude" / "worktrees" / name
    if (tree / ".git").exists():
        git(["git", "-C", str(tree), "checkout", "--quiet", "--detach", sha])
    else:
        git(["git", "-C", str(root), "worktree", "add", "--quiet", "--detach", str(tree), sha])
    return tree


def build_env(env=None) -> dict:
    """The environment a tree builds in: its own `target/`, never a shared one."""
    env = dict(os.environ if env is None else env)
    env.pop("CARGO_TARGET_DIR", None)
    return env


BUILD = ["cargo", "build", "--release", "--locked", "--quiet", "--features", "dev", "--bin", "bench-scenes"]


def build(tree: Path, git=run) -> Path:
    """Build `tree`'s `bench-scenes` (release, `dev`), returning the binary."""
    git(BUILD, cwd=tree, env=build_env())
    return tree / "target" / "release" / "bench-scenes"
