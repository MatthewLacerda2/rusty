"""Which ready pull requests already conflict with `main`, and in which files (#928).

The order (#753) compares pull requests with each other; one that conflicts
with `main` itself can only be handed back, and waiting in line first only
delays that. So the watch asks `git merge-tree` about each head against
`origin/main` before picking, and hands those back at once, naming the files.

**Except what the take would resolve** (#1723): a head whose every conflicted
file the sorted-list rule resolves ([`queue.conflict.resolve`]) is left in line,
or the resolver would never run. The check reads each conflicted file from the
merged tree `merge-tree` writes, with `diff3` so each hunk shows its base: no
worktree, no checkout. It only decides; the take is still the one place that
resolves and pushes, and it records how that ended. A head handed back here is
recorded here ([`queue.conflict.record`]).
"""

from __future__ import annotations

from merging.queue.conflict import record as conflict_record, resolve
from merging.watch import board, order


def paths(output: str) -> list[str]:
    """The conflicted files `git merge-tree --write-tree --name-only --no-messages`
    printed after the tree id."""
    return sorted({line.strip() for line in output.split("\n")[1:] if line.strip()})


def verdicts(status: int, output: str, git) -> dict[str, bool]:
    """Each file `merge-tree` reports conflicted, and whether the sorted-list rule
    resolves it, read from the tree it wrote. Empty when it reports no conflict."""
    if not order.conflicted(status, output):
        return {}
    tree = output.split("\n")[0].strip()

    def shown(path: str) -> str | None:
        status, text = git("cat-file", "-p", f"{tree}:{path}")
        return text if status == 0 else None

    return {path: resolve.decide(path, shown(path)) is not None for path in paths(output)}


def conflicts(pulls: list[dict], git, known: dict[tuple[str, str], list[str]], bots: tuple[str, ...], record=None) -> dict[int, list[str]]:
    """Each of these pull requests that conflicts with `origin/main`, and the files.

    `git(*args)` returns `(status, stdout)`. `main` is fetched once per call;
    `known` remembers each (main, head) pair, so an unchanged poll asks git
    nothing more. Dependabot is left out (it rebases itself). Anything git
    cannot answer fails open: no conflict, and the take finds out as before.
    A head whose every conflicted file resolves is not one ([`verdicts`]).
    `record(number, head, main, paths, ended)`, when given, writes down each
    one handed back ([`queue.conflict.record.note`]).
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
            status, output = git(*resolve.DIFF3, "merge-tree", "--write-tree", "--name-only", "--no-messages", main, key[1])
            found_here = verdicts(status, output, git)
            known[key] = [] if all(found_here.values()) else sorted(found_here)
            if known[key] and record is not None:
                record(pull["number"], key[1], main, found_here, conflict_record.HANDED_BACK)
        if known[key]:
            found[pull["number"]] = known[key]
    return found


def why(files: list[str]) -> str:
    """The hand-back's reason, naming the files."""
    return f"it conflicts with `main` in {', '.join(files)}; merge origin/main into it and push."
