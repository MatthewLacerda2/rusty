"""Ready, labelled pull requests passed over for long on a green head (#1951).

PR #1920 sat ready, labelled `queue` and green for about seven hours on
2026-11-08 because the watch's memory kept it handed back, and nothing said
so after the first `HANDED BACK` line: the orchestrator found it by reading
the log. So `make queue-status` asks, for each hand-back the watch remembers
([`watch.handbacks.MEMORY`]) older than [`STRANDED_SECONDS`]
([`watch.handbacks.WHY_MEMORY`]), whether that pull request is still open,
ready, labelled, on the remembered head, and green ([`watch.stalls.reading`]),
and names each one with the reason it was last handed back. Any kind of
hand-back counts: a green head says CI is no longer what holds it, so the
reason is what to read. GitHub is asked only when an old enough entry exists.
"""

from __future__ import annotations

from pathlib import Path
from typing import Callable

from merging.watch import board, handbacks, stalls

# How long a hand-back may sit on a green head before it is reported.
STRANDED_SECONDS = 3600


def old(memory: dict[int, str], whys: dict[int, dict], now: float) -> list[int]:
    """The remembered hand-backs older than [`STRANDED_SECONDS`]. One with no
    time (remembered before #1951) is old: nothing says otherwise."""
    return [n for n in memory if now - (whys.get(n) or {}).get("at", 0) > STRANDED_SECONDS]


def lines(memory: dict[int, str], whys: dict[int, dict], pulls: list[dict] | None,
          reading: Callable[[dict], tuple[str, str] | None], now: float) -> list[str]:
    """A line per stranded pull request among `pulls` (`None`: GitHub failed);
    `reading(pull)` is [`stalls.reading`] of its head's runs, `None` on a failure."""
    if pulls is None:
        return ["queue-status: GitHub did not list the pull requests; stranded hand-backs not checked."]
    said = []
    for pull in pulls:
        number = pull.get("number")
        if number not in memory or pull.get("isDraft") or not board.queued(pull) or memory[number] != pull.get("headRefOid"):
            continue
        if (reading(pull) or ("",))[0] != stalls.GREEN:
            continue
        why = whys.get(number) or {}
        since = f"{(now - why['at']) / 3600:.1f} h" if "at" in why else "since before its time was kept"
        said.append(f"queue-status: STRANDED #{number}: ready, labelled `queue` and green, passed over {since};"
                    f" last handed back for: {why.get('why', 'a reason not recorded')}")
    return said


def report(memory_path: Path, now: float, listing: Callable[[], list[dict] | None],
           reading: Callable[[dict], tuple[str, str] | None]) -> list[str]:
    """[`lines`] for the memory at `memory_path`; GitHub is asked only when an
    entry is old enough to be stranded."""
    memory = handbacks.recall(memory_path)
    whys = handbacks.recall(memory_path.with_name(handbacks.WHY_MEMORY))
    if not old(memory, whys, now):
        return []
    aged = {n: memory[n] for n in old(memory, whys, now)}
    try:
        listed = listing()
    except SystemExit:  # no repository to ask (`ghrest.slug`): the status itself still stands
        listed = None
    return lines(aged, whys, listed, reading, now)
