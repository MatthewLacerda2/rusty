"""Which ready pull requests already conflict with `main`, and in which files (#928).

The order (#753) compares pull requests with each other; one that conflicts
with `main` itself can only be handed back, and waiting in line first only
delays that. So the watch asks `git merge-tree` about each head against
`origin/main` before picking, and hands those back at once, naming the files.
"""

from __future__ import annotations

from merging.watch import board, order


def paths(output: str) -> list[str]:
    """The conflicted files `git merge-tree --write-tree --name-only --no-messages`
    printed after the tree id."""
    return sorted({line.strip() for line in output.split("\n")[1:] if line.strip()})


def conflicts(pulls: list[dict], git, known: dict[tuple[str, str], list[str]], bots: tuple[str, ...]) -> dict[int, list[str]]:
    """Each of these pull requests that conflicts with `origin/main`, and the files.

    `git(*args)` returns `(status, stdout)`. `main` is fetched once per call;
    `known` remembers each (main, head) pair, so an unchanged poll asks git
    nothing more. Dependabot is left out (it rebases itself). Anything git
    cannot answer fails open: no conflict, and the take finds out as before.
    """
    wanted = [p for p in pulls if not board.is_bot(p, bots) and p.get("headRefOid")]
    if not wanted:
        return {}
    git("fetch", "--quiet", "origin", "main")
    status, main = git("rev-parse", "--verify", "--quiet", "origin/main^{commit}")
    main = main.strip()
    if status != 0 or not main:
        return {}
    missing = [p["headRefOid"] for p in wanted if (main, p["headRefOid"]) not in known
               and git("cat-file", "-e", f"{p['headRefOid']}^{{commit}}")[0] != 0]
    if missing:
        git("fetch", "--quiet", "origin", *missing)
    found = {}
    for pull in wanted:
        key = (main, pull["headRefOid"])
        if key not in known:
            status, output = git("merge-tree", "--write-tree", "--name-only", "--no-messages", main, key[1])
            known[key] = paths(output) if order.conflicted(status, output) else []
        if known[key]:
            found[pull["number"]] = known[key]
    return found


def why(files: list[str]) -> str:
    """The hand-back's reason, naming the files."""
    return f"it conflicts with `main` in {', '.join(files)}; rebase it onto origin/main and push."
