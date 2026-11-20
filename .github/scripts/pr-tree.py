#!/usr/bin/env python3
"""`pr-tree.py N` — check PR #N's head out detached in its own worktree and print its path (#2381).

`make test-gpu PR=<n>` runs its tests there: the head read from origin's
`refs/pull/<n>/head` by sha and checked out in `.claude/worktrees/pr-<n>-head`
(`bench/trees.py`, which writes no local ref another git process could move; the
tree `make bench-ab` measures), built in that tree's own `target/`, and
removed by `make reclaim` once the PR is closed. It is the PR's head as pushed:
no `main` merged in, no timed-run lock. It refuses a tree a live `make bench-ab`
run holds (`trees.held_by_run`): re-pointing it would swap that run's sources
mid-build. It refuses, before fetching, a disk without room for the `dev` test
build (`bench/space.py`, #2436: one such build is about 11 GiB, and the next one
ran a disk to 0 bytes free), naming `make reclaim` and the largest trees; a git
failure on a full disk is said as that. Exits 1 on a refusal or when the fetch or
the checkout fails, printing nothing on stdout, so the recipe stops there; the
reason goes to stderr.

A caller that saw exit 1 and no output at all (#2436, twice) had redirected the
run's output into a file on the full disk: the reason, and make's own error line,
could not be written there. No redirect order fixes that; the space check refuses
while there is still room to say why.
"""

from __future__ import annotations

import subprocess
import sys

from bench import space, trees


def main(argv) -> int:
    if len(argv) != 1 or not argv[0].isdigit():
        print("usage: pr-tree.py PR", file=sys.stderr)
        return 2
    pr = int(argv[0])
    root = None
    try:
        root = trees.main_checkout()
        tree = trees.path(root, trees.pr_tree(pr))
        owner = trees.held_by_run(root, tree)
        if owner is not None:
            raise RuntimeError(f"{tree} is locked by a live `make bench-ab` run ({trees.LOCK_REASON} {owner}): "
                               f"wait for it to end")
        what = f"`make test-gpu PR={pr}`'s dev test build"
        said = space.refusal(root, [(tree, "debug")], what, subprocess.run)
        if said:
            raise RuntimeError(f"refused: {said}")
        _, sha = trees.fetch(pr, again=f"`make test-gpu PR={pr}`")
        trees.checkout(root, trees.pr_tree(pr), sha)
    except RuntimeError as e:
        said = str(e)
        if root is not None and space.said_full(said) and not said.startswith("refused"):
            said = space.full(said, root, subprocess.run)
        print(f"pr-tree: {said}", file=sys.stderr)
        return 1
    print(f"pr-tree: PR #{pr} at {sha[:9]} in {tree}", file=sys.stderr)
    print(tree)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
