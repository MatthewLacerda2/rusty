"""The open pull requests as the watch sees them: labelled, held, changed (#909)."""

from __future__ import annotations

from datetime import datetime


# The listing is [`queue.pulls.open_pulls`] (REST, #970), in GraphQL's field
# names: `files` is what [`held`] reads; `createdAt`, what [`recent`] does.

# The go-ahead (#925): the watch takes a ready pull request only with this label.
QUEUE_LABEL = "queue"

# A bot pull request touching anything under here waits for the orchestrator.
WORKFLOWS = ".github/workflows/"

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
        if not is_bot(p, bots) and any(path in LOCKFILES for path in paths(p))
    )
    return {number: editing[0] for number in editing[1:]}


def lock_line(waiter: int, owner: int) -> str:
    """The one line per waiter and owner, beginning [`LOCKFILE_LINE`]."""
    return f"watch: {LOCKFILE_LINE} #{owner} owns Cargo.lock; #{waiter} waits until it merges or closes."


def queued(pull: dict) -> bool:
    """Whether the orchestrator has given this pull request the go-ahead."""
    return QUEUE_LABEL in names(pull.get("labels"))


def recent(pulls: list[dict], since: datetime | None) -> list[dict]:
    """The pull requests opened at or after `since` (`--since`, #928): the batch's
    own, so the board lines leave older ones out. All of them without it, and a
    pull request GitHub gave no time for is kept."""
    if since is None:
        return pulls
    return [p for p in pulls if not p.get("createdAt") or datetime.fromisoformat(p["createdAt"]) >= since]


def heads(pulls: list[dict]) -> dict[int, tuple[bool, str, bool]]:
    """What counts as movement: a push, a flip between draft and ready, or the
    [`QUEUE_LABEL`] added or removed."""
    return {p.get("number"): (bool(p.get("isDraft")), p.get("headRefOid", ""), queued(p)) for p in pulls}


def transitions(
    before: dict[int, tuple[bool, str, bool]] | None, after: dict[int, tuple[bool, str, bool]], landed: set[int]
) -> list[str]:
    """One line per pull request that changed between two [`heads`] listings
    (#909). `before` is `None` on the first listing, which announces nothing;
    a pull request in `landed` (this watch merged it) is not reported closed.
    Each line has its own word to grep: `opened`, `turned ready`, `back to
    draft`, `closed`, `queued`, `unqueued` (the label, #925)."""
    if before is None:
        return []
    lines = []
    for number in sorted(before.keys() | after.keys()):
        if number not in before:
            lines.append(f"watch: #{number} opened as {'a draft' if after[number][0] else 'ready'}.")
        elif number not in after:
            if number not in landed:
                lines.append(f"watch: #{number} closed, not merged by this watch.")
        elif before[number][0] and not after[number][0]:
            lines.append(f"watch: #{number} turned ready.")
        elif after[number][0] and not before[number][0]:
            lines.append(f"watch: #{number} back to draft.")
        if number in before and number in after and before[number][2] != after[number][2]:
            lines.append(f"watch: #{number} {'queued' if after[number][2] else 'unqueued'}.")
    return lines
