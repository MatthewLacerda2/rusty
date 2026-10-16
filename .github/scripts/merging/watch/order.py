"""Which ready pull request goes next: fewest conflicts in line, then label, then age (#753)."""

from __future__ import annotations

from itertools import combinations

from merging.watch import board


PRIORITY = ("infrastructure", "architecture", "bug", "foundation", "feature")


def rank(
    pull: dict, issue_labels: dict[int, set[str]], bots: tuple[str, ...], counts: dict[int, int] | None = None
) -> tuple[int, int, int, int]:
    """The sort key: Dependabot last, then the fewest conflicts in line
    (`counts`, from [`conflict_counts`]; none given is label order), then the
    best priority label, then age."""
    labels = board.names(pull.get("labels"))
    for issue in pull.get("closingIssuesReferences") or []:
        labels |= issue_labels.get(issue.get("number"), set())
    best = min((PRIORITY.index(label) for label in labels if label in PRIORITY), default=len(PRIORITY))
    number = pull.get("number", 0)
    return int(board.is_bot(pull, bots)), (counts or {}).get(number, 0), best, number


def conflict_counts(numbers: list[int], clashes: set[frozenset[int]]) -> dict[int, int]:
    """How many of the others in `numbers` each one conflicts with; a pair
    naming a pull request no longer in line does not count."""
    present = set(numbers)
    return {n: sum(1 for pair in clashes if n in pair and pair <= present) for n in numbers}


def passed_over(pull: dict, memory: dict[int, str], done: set[int]) -> bool:
    """Whether a ready pull request is skipped: its head was handed back and has
    not moved, or this run already ended it without merging (dry run, `--no-merge`)."""
    number = pull.get("number")
    return number in done or memory.get(number) == pull.get("headRefOid")


def ready(pulls: list[dict], memory: dict[int, str], done: set[int], bots: tuple[str, ...]) -> list[dict]:
    """The pull requests the watch may take now, unordered."""
    return [
        p for p in pulls
        if not p.get("isDraft") and board.queued(p) and p.get("baseRefName", "main") == "main"
        and not passed_over(p, memory, done) and not board.held(p, bots)
    ]


def pick(
    pulls: list[dict],
    issue_labels: dict[int, set[str]],
    memory: dict[int, str],
    done: set[int],
    bots: tuple[str, ...],
    clashes: set[frozenset[int]] = frozenset(),
) -> dict | None:
    """The next pull request to take, or `None`. `clashes` holds the pairs of
    pull requests whose heads conflict ([`clashing`])."""
    waiting = ready(pulls, memory, done, bots)
    counts = conflict_counts([p.get("number") for p in waiting], clashes)
    return min(waiting, key=lambda p: rank(p, issue_labels, bots, counts)) if waiting else None


def reordered(chosen: dict | None, by_label: dict | None, counts: dict[int, int]) -> str | None:
    """One line when the conflict count put `chosen` ahead of `by_label`, the
    pull request labels alone would take; else `None`."""
    if chosen is None or by_label is None or chosen is by_label:
        return None
    first, passed = chosen["number"], by_label["number"]
    return (
        f"watch: #{first} goes before #{passed}, which labels alone would take: #{first} conflicts"
        f" with {counts.get(first, 0)} other(s) in line, #{passed} with {counts.get(passed, 0)}."
    )


def conflicted(status: int, output: str) -> bool:
    """Whether `git merge-tree --write-tree` reported a conflict.

    Exit 1 is not enough: git also exits 1 for a head it cannot find. A real
    conflict prints the merged tree's id first, so that is asked for too;
    anything else fails open.
    """
    first = output.split("\n", 1)[0].strip()
    return status == 1 and len(first) in (40, 64) and all(c in "0123456789abcdef" for c in first)


def clashing(pulls: list[dict], git, known: dict[frozenset[str], bool], bots: tuple[str, ...]) -> set[frozenset[int]]:
    """The pairs of these pull requests whose heads textually conflict.

    `git(*args)` returns `(status, stdout)`. Dependabot is left out ([`rank`]).
    `known` remembers each pair of *heads*, so a poll that sees the same heads
    asks git nothing. Heads this checkout lacks are fetched by sha, once,
    together; a failed fetch is only more pairs that fail open ([`conflicted`]).
    """
    heads = {p["number"]: p.get("headRefOid", "") for p in pulls if not board.is_bot(p, bots)}
    pairs = [(a, b) for a, b in combinations(sorted(heads), 2) if heads[a] and heads[b]]

    def key(a: int, b: int) -> frozenset[str]:
        return frozenset((heads[a], heads[b]))

    unknown = [(a, b) for a, b in pairs if key(a, b) not in known]
    if unknown:
        wanted = sorted({heads[n] for pair in unknown for n in pair})
        missing = [sha for sha in wanted if git("cat-file", "-e", f"{sha}^{{commit}}")[0] != 0]
        if missing:
            git("fetch", "--quiet", "origin", *missing)
        for a, b in unknown:
            known[key(a, b)] = conflicted(*git("merge-tree", "--write-tree", heads[a], heads[b]))
    return {frozenset(pair) for pair in pairs if known.get(key(*pair), False)}
