"""The open pull requests as the watch sees them: labelled, held, changed (#909)."""

from __future__ import annotations

import re

from datetime import datetime


# The listing is [`queue.pulls.open_pulls`] (REST, #970), in GraphQL's field
# names: `files` is what [`held`] reads; `createdAt`, what [`fresh`] does.

# The go-ahead by hand (#925): the watch takes a ready pull request with this
# label whatever its verdict ([`queued`], #2628).
QUEUE_LABEL = "queue"

# The keep-out by hand (#2628): no pull request with it is taken, whatever its label or verdict.
HOLD_LABEL = "hold"

# A bot pull request touching anything under here waits for the orchestrator:
# the workflows, and the composite actions they call (#1973).
WORKFLOWS = (".github/actions/", ".github/workflows/")

# How the line saying so begins ([`held`]): not a hand-back, nothing to wake for.
HELD_LINE = "HELD"


def names(labels) -> set[str]:
    """Label names from `gh`'s `[{"name": ...}]`."""
    return {label.get("name", "") for label in labels or []}


def is_bot(pull: dict, bots: tuple[str, ...]) -> bool:
    return (pull.get("author") or {}).get("login") in bots


def held(pull: dict, bots: tuple[str, ...]) -> list[str]:
    """The workflows a Dependabot pull request edits, which keep the watch off
    it (#825); empty for anyone else's, or a bot's that edits none."""
    if not is_bot(pull, bots):
        return []
    return sorted(p for p in paths(pull) if p.startswith(WORKFLOWS))


def held_line(pull: dict, workflows: list[str]) -> str:
    """The one line a held pull request gets per head, beginning [`HELD_LINE`]."""
    return (
        f"{HELD_LINE} #{pull.get('number')}: Dependabot edits {', '.join(workflows)}, which a pull-request"
        " run does not prove. Review it, dispatch those workflows on its branch, then `make queue PRS=N`."
    )


# The files one pull request at a time may edit (#969): two branches resolving
# dependencies separately conflict in `Cargo.lock`, which is never hand-merged
# (`ci-merge`). On 2026-10-04 seven lockfile PRs merged within five hours and
# #767 was handed back three times.
LOCKFILES = ("Cargo.toml", "Cargo.lock")
# A workspace member's manifest resolves into the same lockfile (crates/CLAUDE.md).
MEMBER_MANIFEST = re.compile(r"crates/[^/]+/Cargo\.toml")

# How the line saying who waits on whom begins.
LOCKFILE_LINE = "LOCKFILE"


def paths(pull: dict) -> list[str]:
    """The files a pull request edits, from `gh`'s `files`."""
    return [f.get("path", "") for f in pull.get("files") or []]


def lock_waits(pulls: list[dict], bots: tuple[str, ...]) -> dict[int, int]:
    """Each pull request held behind an older one that edits a lockfile too,
    mapped to that owner (#969). Drafts count, as owner and as waiter: the
    owner's resolution is the one the rest merge in. Dependabot is
    neither: it rebases itself."""
    editing = sorted(
        p.get("number") for p in pulls
        if not is_bot(p, bots) and any(path in LOCKFILES or MEMBER_MANIFEST.fullmatch(path) for path in paths(p))
    )
    return {number: editing[0] for number in editing[1:]}


def lock_line(waiter: int, owner: int) -> str:
    """The one line per waiter and owner, beginning [`LOCKFILE_LINE`]."""
    return f"watch: {LOCKFILE_LINE} #{owner} owns Cargo.lock; #{waiter} waits until it merges or closes."


def queued(pull: dict) -> bool:
    """Whether this pull request has the go-ahead (#2628): not [`HOLD_LABEL`]led,
    nothing owed (a `Needs:` no `Done:` answered), and either the
    [`QUEUE_LABEL`] or a `queue` verdict that holds on its head. `standing` is
    [`verdict.standing.standing`], attached by the listing
    ([`queue.verdicts.Reader`]); a pull request without one is read by its
    labels alone, as before #2628, and one whose re-read before a merge failed
    ([`queue.verdicts.UNREAD`]) has none, so it goes back in line."""
    labels = names(pull.get("labels"))
    standing = pull.get("standing") or {}
    if HOLD_LABEL in labels or standing.get("owed") or standing.get("unread"):
        return False
    return QUEUE_LABEL in labels or standing.get("verdict") == "queue"


def lost(pull: dict) -> str:
    """Why a pull request [`queued`] refuses lost its go-ahead, for the line that
    puts it back in line."""
    if (pull.get("standing") or {}).get("unread"):
        return "GitHub did not show its comments when its go-ahead was asked again, so it is read on the next pass"
    return f"it lost its go-ahead (the `{QUEUE_LABEL}` label or verdict)"


def fresh(when: str | None, since: datetime | None) -> bool:
    """Whether a pull request opened at `when` is said under `--since` (#928):
    the batch's own, so the board lines leave older ones out. Every one without
    `--since`, and one GitHub gave no time for ([`kept.Board.report`])."""
    return since is None or not when or datetime.fromisoformat(when) >= since


def heads(pulls: list[dict]) -> dict[int, tuple[bool, str, bool]]:
    """What counts as movement: a push, a flip between draft and ready, or the
    go-ahead given or taken back ([`queued`]: a label, a verdict, a `Needs:`)."""
    return {p.get("number"): (bool(p.get("isDraft")), p.get("headRefOid", ""), queued(p)) for p in pulls}


def opened(pulls: list[dict]) -> dict[int, str | None]:
    """When each pull request opened, what `--since` reads ([`fresh`]); kept
    beside [`heads`] so a closed one is still filtered by it ([`kept`], #2215)."""
    return {p.get("number"): p.get("createdAt") for p in pulls}


def transitions(
    before: dict[int, tuple[bool, str, bool]] | None, after: dict[int, tuple[bool, str, bool]], landed: set[int],
    about: set[int] | None = None, closes: dict[int, bool | None] | None = None,
) -> list[str]:
    """One line per pull request that changed between two [`heads`] listings
    (#909); a pull request in `landed` (this watch merged it) is not reported
    closed. Each line has its own word to grep: `opened`, `turned ready`, `back
    to draft`, `closed` or `merged`, `queued`, `unqueued` (the label, #925).
    Only numbers in `about` are said, when given (`--since`, [`fresh`]).

    A watch's first listing is compared with the last one any watch told
    ([`kept`], #2215), so what moved between two watches is said once by the
    second; `closes` holds what GitHub said of each number gone between them
    ([`closed_line`]). `before` is `None` only when nothing was kept (a fresh
    start, an unreadable file): then each ready pull request without the label
    is said to be waiting ([`waiting_line`]), and nothing else, since a draft
    or a queued one asks nothing of the orchestrator."""
    said = (lambda number: True) if about is None else about.__contains__
    if before is None:
        return [waiting_line(n) for n in sorted(after) if said(n) and not after[n][0] and not after[n][2]]
    lines = []
    for number in sorted(before.keys() | after.keys()):
        if not said(number):
            continue
        if number not in before:
            lines.append(f"watch: #{number} opened as {'a draft' if after[number][0] else 'ready'}.")
        elif number not in after:
            if number not in landed:
                lines.append(closed_line(number, closes))
        elif before[number][0] and not after[number][0]:
            lines.append(f"watch: #{number} turned ready.")
        elif not before[number][0] and after[number][0]:
            lines.append(f"watch: #{number} back to draft.")
        if number in before and number in after and before[number][2] != after[number][2]:
            lines.append(f"watch: #{number} {'queued' if after[number][2] else 'unqueued'}.")
    return lines


def closed_line(number: int, closes: dict[int, bool | None] | None) -> str:
    """A pull request gone from the listing. Inside one watch, "not merged by
    this watch" is all it knows; between watches ([`transitions`]' `closes`)
    GitHub was asked, since a merge while no watch ran (`make queue PRS=N`, by
    hand) is a merge (#2215). It names no merger: a merge this watch made can
    reach the next watch's first listing too, when power is cut between the
    board's write and the hand-back memory's ([`kept`])."""
    if closes is None or number not in closes:
        return f"watch: #{number} closed, not merged by this watch."
    if closes[number] is None:
        return f"watch: #{number} closed (merged or not: GitHub did not answer)."
    return f"watch: #{number} {'merged while no watch listed it' if closes[number] else 'closed, not merged'}."


def waiting_line(number: int) -> str:
    """A ready, unlabelled pull request a fresh start found: whether it was
    ever announced is unknown, so it is said now ([`transitions`], #2215)."""
    return f"watch: #{number} is ready and unqueued; no earlier listing was kept to compare."
