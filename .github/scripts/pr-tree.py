#!/usr/bin/env python3
"""`pr-tree.py N` — check PR #N's head out detached in its own worktree and print its path (#2381).

`make test-gpu PR=<n>` runs its tests there: the head read from origin's
`refs/pull/<n>/head` by sha and checked out in `.claude/worktrees/pr-<n>-head`
(`bench/trees.py`, which writes no local ref another git process could move; the
tree `make bench-ab` measures), built in that tree's own `target/`, and
removed by `make reclaim` once the PR is closed. It is the PR's head as pushed:
no `main` merged in, no timed-run lock. It refuses a tree a live `make bench-ab`
run holds (`trees.held_by_run`): re-pointing it would swap that run's sources
mid-build. Exits 1 on that refusal or when the fetch or the checkout fails,
printing nothing on stdout, so the recipe stops there.
"""

from __future__ import annotations

import sys

from bench import trees


def main(argv) -> int:
    if len(argv) != 1 or not argv[0].isdigit():
        print("usage: pr-tree.py PR", file=sys.stderr)
        return 2
    pr = int(argv[0])
    try:
        root = trees.main_checkout()
        owner = trees.held_by_run(root, trees.path(root, trees.pr_tree(pr)))
        if owner is not None:
            raise RuntimeError(f"{trees.path(root, trees.pr_tree(pr))} is locked by a live `make bench-ab` "
                               f"run ({trees.LOCK_REASON} {owner}): wait for it to end")
        _, sha = trees.fetch(pr, again=f"`make test-gpu PR={pr}`")
        tree = trees.checkout(root, trees.pr_tree(pr), sha)
    except RuntimeError as e:
        print(f"pr-tree: {e}", file=sys.stderr)
        return 1
    print(f"pr-tree: PR #{pr} at {sha[:9]} in {tree}", file=sys.stderr)
    print(tree)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
