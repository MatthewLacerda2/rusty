"""`merge-queue.py --watch` — take each pull request as it turns ready (#664).

A cloud coder's only report is its pull request, so "done" is visible only as
the flip from draft to ready. Before this, the orchestrator noticed each flip
with a hand-rolled poll and chained one `make queue PRS=N` per pull request —
about 69 queue runs and dozens of ad-hoc watchers in the 2026-09-30 batch.
`--watch` does that noticing: it lists the open pull requests, takes the
highest-priority ready one through the queue's unchanged pipeline
([`merge-queue.take`]), and lists again.

Still invoked, never a service: somebody starts it, it runs on this machine,
and it **exits on the first thing that needs judgement**, so whoever started it
in the background is woken:

- **a hand-back** (conflict, failed local check, red CI, no run, a refused
  merge). It exits rather than skipping on: the hand-back is the report, and
  the ready pull requests behind it wait for the next start, a few minutes
  away. The handed-back head is remembered ([`MEMORY`]) and passed over until
  it moves, so restarting the watch never re-takes an unfixed branch; a fix
  pushed to it makes it eligible again. `make queue PRS=N` ignores the memory;
- **the machine failing** (#584's stop, or GitHub unreachable for [`FAILS`]
  listings in a row);
- **nothing left**: no open pull request, or none ready and no head among the
  rest (drafts, remembered hand-backs) has moved for `--idle` minutes.

Which ready one goes first follows CLAUDE.md's priority, read from the
pull request's own labels and the issues it closes (rusty labels issues, not
pull requests): infrastructure → architecture → bug → foundation → feature,
then anything else, oldest number first within a rank. Dependabot goes after
every one of those. A pull request whose base is not `main` is never taken:
the queue rebases onto `main`.

The decisions are pure functions over plain dictionaries; [`run`] takes every
effect as an argument, so the loop is tested without a network.
"""

from __future__ import annotations

import json
from pathlib import Path

PRIORITY = ("infrastructure", "architecture", "bug", "foundation", "feature")

# How long nothing moving among the drafts reads as "nothing left". A coder
# pushes often (cloud-brief.md), but a cold build and `make gates` can run an
# hour between pushes; past this the orchestrator is better woken than not.
IDLE_MINUTES = 90

# Consecutive listings GitHub may fail before the watch stops as the machine's
# problem rather than retrying forever. At the default poll, five minutes.
FAILS = 10

# What `gh pr list` is asked for, and the issues whose labels give the priority.
LIST_FIELDS = "number,isDraft,headRefOid,baseRefName,author,labels,closingIssuesReferences"

# Where the remembered hand-backs live, under the checkout's git directory.
MEMORY = "merge-queue/handed-back.json"


def names(labels) -> set[str]:
    """Label names from `gh`'s `[{"name": ...}]`."""
    return {label.get("name", "") for label in labels or []}


def rank(pull: dict, issue_labels: dict[int, set[str]], bots: tuple[str, ...]) -> tuple[int, int, int]:
    """The sort key: Dependabot last, then the best priority label, then age."""
    labels = names(pull.get("labels"))
    for issue in pull.get("closingIssuesReferences") or []:
        labels |= issue_labels.get(issue.get("number"), set())
    best = min((PRIORITY.index(label) for label in labels if label in PRIORITY), default=len(PRIORITY))
    bot = (pull.get("author") or {}).get("login") in bots
    return int(bot), best, pull.get("number", 0)


def passed_over(pull: dict, memory: dict[int, str], done: set[int]) -> bool:
    """Whether a ready pull request is skipped: its head was handed back and has
    not moved, or this run already ended it without merging (dry run, `--no-merge`)."""
    number = pull.get("number")
    return number in done or memory.get(number) == pull.get("headRefOid")


def ready(pulls: list[dict], memory: dict[int, str], done: set[int]) -> list[dict]:
    """The pull requests the watch may take now, unordered."""
    return [
        p for p in pulls
        if not p.get("isDraft") and p.get("baseRefName", "main") == "main" and not passed_over(p, memory, done)
    ]


def pick(
    pulls: list[dict], issue_labels: dict[int, set[str]], memory: dict[int, str], done: set[int], bots: tuple[str, ...]
) -> dict | None:
    """The next pull request to take, or `None`."""
    waiting = ready(pulls, memory, done)
    return min(waiting, key=lambda p: rank(p, issue_labels, bots)) if waiting else None


def heads(pulls: list[dict]) -> dict[int, tuple[bool, str]]:
    """What counts as movement: a push, or a flip between draft and ready."""
    return {p.get("number"): (bool(p.get("isDraft")), p.get("headRefOid", "")) for p in pulls}


def finished(pulls: list[dict], quiet: float, idle: float) -> str | None:
    """Why to exit with nothing to take, or `None` to keep watching.

    Called only when [`pick`] found nothing. `quiet` is how long, in seconds,
    no [`heads`] have changed.
    """
    if not pulls:
        return "no open pull request is left."
    if quiet >= idle:
        return (
            f"nothing is ready, and none of the {len(pulls)} open pull request(s) has moved"
            f" in {idle / 60:.0f} minutes: " + ", ".join(f"#{p.get('number')}" for p in pulls) + "."
        )
    return None


def forget_closed(memory: dict[int, str], pulls: list[dict]) -> dict[int, str]:
    """The memory without pull requests that are no longer open."""
    open_ = {p.get("number") for p in pulls}
    return {n: sha for n, sha in memory.items() if n in open_}


def recall(path: Path) -> dict[int, str]:
    """The remembered hand-backs; a missing or unreadable file is none."""
    try:
        return {int(n): sha for n, sha in json.loads(path.read_text()).items()}
    except (OSError, ValueError, AttributeError):
        return {}


def remember(path: Path, memory: dict[int, str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({str(n): sha for n, sha in sorted(memory.items())}, indent=1) + "\n")


def run(fx, opts) -> tuple[list[tuple[int, str, str]], str, bool]:
    """Watch until something needs a person. Returns the results, why it ended,
    and whether that ending is a clean one (nothing left, or a dry run done).

    `fx` carries every effect: `pulls()` (the open listing, or `None` when GitHub
    failed), `issue_labels()`, `turn(number)` (one pull request through the
    queue, as [`merge-queue.take`]), `head(number)`, `clock()`, `sleep(seconds)`,
    `say(...)`, `memory` (a path), `bots`, and the end states: `merged`, `ends`
    (ended without a hand-back: green under `--no-merge`, previewed by a dry run)
    `stops` (the machine's failure, [`merge-queue.Stopped`]) and `skips` (it
    turned draft or closed after the listing). Anything else is a hand-back.
    """
    results, done, failed = [], set(), 0
    memory = recall(fx.memory)
    last, moved = None, fx.clock()
    while True:
        pulls = fx.pulls()
        if pulls is None:
            failed += 1
            if failed >= FAILS:
                return results, f"GitHub failed {FAILS} listings in a row; stopped.", False
            fx.sleep(opts.poll)
            continue
        failed = 0
        memory = forget_closed(memory, pulls)
        now = heads(pulls)
        if now != last:
            last, moved = now, fx.clock()
        labels = fx.issue_labels() if len(ready(pulls, memory, done)) > 1 else {}
        chosen = pick(pulls, labels, memory, done, fx.bots)
        if chosen is None:
            why = finished(pulls, fx.clock() - moved, opts.idle * 60)
            if opts.dry_run and not why:
                why = "dry run: every ready pull request has been previewed."
            if why:
                remember(fx.memory, memory)
                return results, why, True
            fx.sleep(opts.poll)
            continue
        number = chosen["number"]
        fx.say(f"watch: taking #{number}.")
        result = fx.turn(number)
        results.append(result)
        state = result[1]
        if state == fx.merged:
            memory.pop(number, None)
        elif state in fx.ends:
            done.add(number)
        elif state in fx.skips:
            fx.sleep(opts.poll)
        elif state in fx.stops:
            remember(fx.memory, memory)
            return results, f"the machine failed on #{number}, not the branch; nothing is remembered against it.", False
        else:
            memory[number] = fx.head(number) or chosen.get("headRefOid", "")
            remember(fx.memory, memory)
            return results, (
                f"#{number} was handed back. Its head is passed over until it moves;"
                f" `make queue PRS={number}` takes it regardless."
            ), False
        remember(fx.memory, memory)
        last = None  # the merge moved `main` and the listing; look afresh
