"""How many cloud coders are running, and whether one more may start (#1856).

The operator set the limit on 2026-11-05: **at most [`LIMIT`] cloud coders at
once** (CLAUDE.md, *Coding parallelises; merging does not*). Review, not coder
count, was a batch's bottleneck, so a sixth coder only queues more diffs for
one orchestrator to read.

**The routines API cannot tell a working coder from a finished one** (#1866).
A one-off routine not fired yet shows `enabled` true, no `ended_reason` and no
`last_run`. Once fired, its `last_run.status` reads `SUCCEEDED` with a
`finished_at` whenever the session ends a turn, and a coder waiting on a
background command (a dispatched mutation run) ends its turn and wakes later:
on 2026-11-07, within the hour #1856's gate merged, #1844's and #1714's coders
read `ended` while working, so with five coders running the gate saw two. The
session listing doesn't help: a finished coder and a waiting one both show
`status: active`, `worker_status: idle`.

**So a fired coder is placed from GitHub**, since a coder's session ends when
its pull request is ready (`cloud-brief.md`). For each issue's newest
`rusty #N` routine:

- not fired yet → running;
- fired, and its fire time cannot be read → *unknown*: named, not counted;
- fired, and an open pull request for N (its branch `N-…`, CLAUDE.md *Branch
  naming*) is ready → running, *in review* ([`REVIEWING`]), until a comment on
  it records the review decision (`merging/verdict/reviewed.py`'s test: first
  line `Independent review…` or `No review: <why>`, by someone who can push),
  then ended, whatever the record's outcomes say (#2017). On 2026-11-10 the slot
  freed at ready went to a new issue twice (#2009, #2015), and the review then
  sent each PR back with no slot for its re-brief; most reviews send a PR back
  (48 of 57 re-briefs that batch), and a re-brief finishes something nearly
  done. The re-brief takes exactly the held slot, since the issue being
  launched is never counted;
- fired, and N's pull request is a draft, **or** N has no open pull request,
  the issue is still open and the fire is under [`FIRST_PUSH`] old (a coder
  that has not opened its PR yet) → running, unless a `Brief` on N from someone
  who can push (`ghrest.can_push`) is newer than the fire. The orchestrator
  posts a re-brief, then launches, so a newer brief is a hand-back waiting for
  its relaunch, with no coder on it;
- fired, no open pull request, the issue open, and a pull request for N
  merged or closed after the fire → ended (#1913). A PR that says `Refs #N`
  leaves its issue open: on 2026-11-08 #1684's PR #1903 merged at ≈05:40Z and
  its coder still held a slot until 06:50Z, refusing two re-briefs with four
  coders really running. A pull request closed *before* the fire is an earlier
  round's, so it says nothing about this coder;
- fired, no open pull request, none closed since the fire, the issue open and
  the fire [`FIRST_PUSH`] old or more → ended, and said (`STALE`): a coder
  that stood down or stalled before its first push, or one whose merged pull
  request is past the one page of closed ones read ([`GitHub.closed`]);
- fired, no open pull request, and the issue closed (merged) → ended.

A stalled coder (draft, no newer brief) counts until the orchestrator deals
with it, which is the right pressure; the refusal names every counted routine
and when it fired, and `--over-limit` launches anyway. The routines list holds
only the account's 20 newest routines (`routine.ENVIRONMENT_FROM`), so a coder
pushed off it by 20 newer ones is not seen.

A re-brief is not one more coder: the issue being launched is left out of
the count, so its own earlier routine never blocks it.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from datetime import datetime, timedelta
from typing import Callable

import ghrest
from dispatch import brief, routine
from merging.verdict import reviewed

#: The most cloud coders at once (the operator, 2026-11-05).
LIMIT = 5

#: How long a fired coder may go without an open pull request and still count: a
#: coder opens its draft on its first commit (`cloud-brief.md`, *Protocol*), well
#: inside this. Past it, a fired coder with no PR on an open issue has ended or
#: stalled, and counting it would hold a slot forever (#1866's review).
FIRST_PUSH = timedelta(hours=2)
STALE = "no PR in 2 h: ended or stalled"
#: Why a ready pull request with no review record still counts (#2017).
REVIEWING = "in review"

ISSUE = re.compile(re.escape(routine.NAME_PREFIX) + r"(\d+)\b")
BRANCH = re.compile(r"^(\d+)-")


@dataclass
class GitHub:
    """What placing a fired coder reads: the open pull requests, per issue its comments,
    itself and its closed pull requests, and the time now (to age a fire against
    [`FIRST_PUSH`]). `closed` is read only for an issue with no open pull request; the
    caller may answer it from one page of the newest closed pull requests (#1913)."""

    pulls: list[dict]
    comments: Callable[[int], list | None]
    issue: Callable[[int], dict | None]
    now: datetime
    closed: Callable[[int], list | None]


def issue(found: dict) -> int | None:
    """The issue a coder routine is for, from its name; `None` for any other routine."""
    match = ISSUE.match(str(found.get("name", "")))
    return int(match.group(1)) if match else None


def fired(found: dict) -> bool:
    return bool(found.get("last_run") or found.get("last_fired_at") or found.get("ended_reason"))


def fired_at(found: dict) -> datetime | None:
    """When the routine fired, or `None` when the listing doesn't say."""
    run = found.get("last_run") if isinstance(found.get("last_run"), dict) else {}
    return when(run.get("fired_at") or found.get("last_fired_at"))


def when(stamp: object) -> datetime | None:
    try:
        return datetime.fromisoformat(str(stamp).replace("Z", "+00:00")) if stamp else None
    except ValueError:
        return None


def pulls_for(number: int, pulls: list[dict]) -> list[dict]:
    """The open pull requests whose branch is `number-…`."""
    return [p for p in pulls if (m := BRANCH.match(str((p.get("head") or {}).get("ref", ""))))
            and int(m.group(1)) == number]


def closed_since(pulls: list[dict], since: datetime) -> dict | None:
    """The newest of `pulls` closed (merged or not) after `since`, or `None`."""
    after = [p for p in pulls if (at := when(p.get("closed_at"))) is not None and at > since]
    return max(after, key=lambda p: when(p["closed_at"]), default=None)


def rebriefed(comments: list[dict], since: datetime) -> bool:
    """Whether a `Brief` from someone who can push was posted after `since`."""
    trusted = [c for c in comments if ghrest.can_push(c)]
    newest = brief.newest(trusted)
    posted = when(newest.get("created_at")) if newest else None
    return posted is not None and posted > since


def state(found: dict, github: GitHub) -> tuple[str, str]:
    """`running`, `ended` or `unknown`, and why, for one issue's newest coder routine."""
    if not fired(found):
        return ("running", "not fired") if found.get("enabled") else ("ended", "disabled")
    since = fired_at(found)
    if since is None:
        return "unknown", "fired, no fire time listed"
    number = issue(found)
    mine = pulls_for(number, github.pulls)
    if mine and not any(p.get("draft") for p in mine):
        return in_review(mine, github)
    if not mine:
        got = github.issue(number)
        if got is None:
            return "unknown", f"could not read #{number}"
        if got.get("state") != "open":
            return "ended", f"#{number} is closed"
        shut = github.closed(number)
        if shut is None:
            return "unknown", f"could not read #{number}'s closed pull requests"
        if (done := closed_since(shut, since)) is not None:
            return "ended", "its pull request merged" if done.get("merged_at") else "its pull request closed"
        if github.now - since >= FIRST_PUSH:
            return "ended", STALE
    comments = github.comments(number)
    if comments is None:
        return "unknown", f"could not read #{number}'s comments"
    if rebriefed(comments, since):
        return "ended", "a newer brief waits for its relaunch"
    return "running", "draft pull request" if mine else "no pull request yet"


def in_review(ready: list[dict], github: GitHub) -> tuple[str, str]:
    """A fired coder whose pull requests are all ready: running while one has no review record
    (`reviewed.decided`), since the review most often sends it back to its coder (#2017)."""
    for pull in ready:
        comments = github.comments(pull.get("number"))
        if comments is None:
            return "unknown", f"could not read #{pull.get('number')}'s comments"
        if not reviewed.decided(comments):
            return "running", REVIEWING
    return "ended", "its pull request is ready and reviewed"


def newest(routines: list[dict], launching: int) -> list[dict]:
    """Each issue's newest coder routine, leaving out `launching` (a re-brief)."""
    latest: dict[int, dict] = {}
    for found in routines:
        number = issue(found)
        if number is None or number == launching:
            continue
        if number not in latest or found.get("created_at", "") > latest[number].get("created_at", ""):
            latest[number] = found
    return list(latest.values())


def coders(routines: list[dict], launching: int,
           github: GitHub) -> tuple[list[tuple[dict, str]], list[tuple[dict, str, str]]]:
    """The coder routines that count, as (routine, why), and the uncounted ones worth saying,
    as (routine, placement, why): those that cannot be placed, and those [`STALE`] past
    [`FIRST_PUSH`]."""
    counted, noted = [], []
    for found in newest(routines, launching):
        placed, why = state(found, github)
        if placed == "running":
            counted.append((found, why))
        elif placed == "unknown" or why == STALE:
            noted.append((found, placed, why))
    return counted, noted


def describe(found: dict, why: str = "") -> str:
    """One counted coder for the refusal: its name, when it fired or fires, and `in review`
    for a ready pull request still waiting on its review record ([`REVIEWING`])."""
    since = fired_at(found)
    told = f"fired {since:%Y-%m-%dT%H:%M}Z" if since and fired(found) else f"fires {found.get('run_once_at', '?')}"
    if why == REVIEWING:
        told += f", {REVIEWING}"
    return f"{found.get('name', '?')} ({told})"
