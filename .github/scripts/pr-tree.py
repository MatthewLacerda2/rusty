#!/usr/bin/env python3
"""`pr-tree.py N` — check PR #N's head out detached in its own worktree and print its path (#2381).

`make test-gpu PR=<n>` runs its tests there: the head fetched as
`pull/<n>/head` into `.claude/worktrees/pr-<n>-head` (`bench/trees.py`, the
tree `make bench-ab` measures), built in that tree's own `target/`, and
removed by `make reclaim` once the PR is closed. Exits 1 when the fetch or the
checkout fails, printing nothing on stdout, so the recipe stops there.
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
        _, sha = trees.fetch(pr)
        tree = trees.checkout(trees.main_checkout(), trees.pr_tree(pr), sha)
    except RuntimeError as e:
        print(f"pr-tree: {e}", file=sys.stderr)
        return 1
    print(f"pr-tree: PR #{pr} at {sha[:9]} in {tree}", file=sys.stderr)
    print(tree)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
