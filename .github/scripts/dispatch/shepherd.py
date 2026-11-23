"""The shepherd's routine: one fresh cloud session per turn of a pull request's review loop (#2629).

The orchestrator spent four or five turns per pull request on its review loop
(dispatch the reviewer, check each finding, post the record, write the round's
`Brief`, read the round, post the outcomes): routine judgement the operator
gave to Opus. So a cloud routine does it, started by `make shepherd PR=N`,
which the quiet watch runs when a pull request turns ready with no standing
verdict (`merging/view/shepherding.py`):

- **A cloud routine, not a local headless session**: the operator's machine
  loses power without warning and a local session dies with it; a routine does
  not, and [`routine`]'s launch path already exists. A routine's session is a
  full Claude Code cloud session (skills, the GitHub MCP tools, subagents), so
  [`ALLOWED_TOOLS`] is the coder's list plus `Agent`, which `code-review` uses.
- **The prompt is a pointer** ([`PROMPT`]): the agent definition carries the
  steps, the pull request is read from GitHub. The session never sees a brief.
- **Its name is not a coder's** ([`NAME_PREFIX`]): `running` counts every
  routine named `rusty #`, so a shepherd named that way would take a coder's
  slot.
- **At most [`CAP`] at once, and one per pull request** ([`in_flight`]): a
  shepherd routine counts while it is under [`BUSY`] old and its pull request
  has no record posted since it was made. The routines API cannot tell a
  working session from a finished one (`running`'s docstring), and a record is
  what a shepherd's turn ends in; a turn that posts nothing (a stop) frees its
  count when [`BUSY`] passes. A second start on a pull request with one in
  flight is refused, so a replayed ready line never starts two.
"""

from __future__ import annotations

from datetime import datetime, timedelta, timezone

from dispatch import routine
from dispatch.model import MODEL
from merging.verdict import standing

PROMPT = ("Read `.claude/agents/shepherd.md` and follow it.\n"
          "Pull request: #{number} of {repository}.")

ALLOWED_TOOLS = [*routine.ALLOWED_TOOLS, "Agent"]

#: Every shepherd routine's name starts with this, then the pull request.
NAME_PREFIX = "rusty shepherd #"

#: How many shepherds run at once: a review is a few minutes of reading, so two keep up with five coders.
CAP = 2

#: How long a shepherd routine counts without a record: a high review takes about half an hour.
BUSY = timedelta(minutes=75)


def name(number: int, title: str) -> str:
    """`rusty shepherd #N <title>`, cut at a word to `routine.NAME_LENGTH`."""
    full = f"{NAME_PREFIX}{number} {' '.join(title.split())}"
    return full if len(full) <= routine.NAME_LENGTH else full[:routine.NAME_LENGTH].rsplit(" ", 1)[0]


def number_of(listed: dict) -> int | None:
    """The pull request a shepherd routine names, or `None` for any other routine."""
    rest = str(listed.get("name", ""))
    if not rest.startswith(NAME_PREFIX):
        return None
    digits = rest[len(NAME_PREFIX):].split(" ", 1)[0]
    return int(digits) if digits.isdigit() else None


def stamp(text: str | None) -> datetime | None:
    try:
        return datetime.fromisoformat(str(text).replace("Z", "+00:00")).astimezone(timezone.utc)
    except ValueError:
        return None


def in_flight(listed: list[dict], comments, now: datetime) -> dict[int, dict]:
    """The pull requests with a shepherd still counted, and its routine: under [`BUSY`]
    old with no trusted record since it was made. `comments(n)` is pull request `n`'s
    comments, `None` when GitHub did not answer (then it counts: a doubt holds the slot)."""
    found: dict[int, dict] = {}
    for made in sorted(listed, key=lambda r: r.get("created_at") or ""):
        number, at = number_of(made), stamp(made.get("created_at"))
        if number is None or at is None or now - at >= BUSY:
            continue
        read = comments(number)
        posted = read is not None and any(
            (stamp(c.get("created_at")) or at) > at for c in standing.records(read))
        if not posted:
            found[number] = made
    return found


def body(number: int, title: str, environment_id: str, repository: str, now: datetime,
         event_id: str | None = None) -> dict:
    """The `create` body for the shepherd of pull request `number`, on the latest Opus."""
    return routine.job(name(number, title), PROMPT.format(number=number, repository=repository),
                       environment_id, repository, now, event_id, MODEL, ALLOWED_TOOLS)
