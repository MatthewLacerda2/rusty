"""Coders' comments on their batch issues, for `make queue-events` (#1876, #2136).

A cloud coder that cannot start (an open blocker) or hands its work back (a
decision it cannot make) says so on its issue, not on a pull request, so the
watch's log never sees it, and a draft that stays a draft raises no event
either. This is the light poll beside the log: the repository's issue comments
updated since the stream began, less [`LOOKBACK_MINUTES`], kept when the author
can push (`ghrest.can_push`: the repository is public) and the issue is the
batch's. Each comment is said once per stream.

Two kinds pass:

- **A stand-down** (first line `Standing down`, the wording `cloud-brief.md`
  prefers): `STANDING DOWN #N …`, on an issue with a `Brief` or an open pull
  request that closes it ([`in_batch`]).
- **Any other comment** on an issue with a trusted `Brief`: `COMMENT #N …`.
  Until #2136 only a stand-down passed, and on 2026-11-11 a hand-back worded
  "Handed back on draft PR #2104: …" sat unread for three hours. A coder
  comments on a batch issue only to stand down or hand back, so whatever the
  wording, the comment is an event. An issue with only a pull request (no
  `Brief`) is not a coder's, so its discussion stays out.

The orchestrator comments as the same account as every coder, so its own
comments are told apart by their first line, not their author ([`OWN`]): a
`Brief` and a `Context update` are skipped. A note of its own under any other
heading passes: a spurious line costs a glance, a missed hand-back hours.

The look back is the log's 30 lines in another form: a `Monitor` is re-armed
every 30 minutes, and a stream that began only at its own start would drop a
comment posted between two of them. A comment said again by the next stream is
cheaper than one never said.
"""

from __future__ import annotations

import json
import re
import subprocess
from datetime import datetime, timedelta, timezone
from typing import Callable

import ghrest
from dispatch import brief

LOOKBACK_MINUTES = 35

FIRST_LINE = re.compile(r"^\W*Standing down\b", re.IGNORECASE)

# The orchestrator's own comments on a batch issue, by first line.
OWN = re.compile(r"^\W*(Brief|Context update)\b", re.IGNORECASE)

# What [`in_batch`] answers: a trusted `Brief` on the issue, or only an open
# pull request closing it.
BRIEFED, CLOSED_BY_PULL = "briefed", "closed by a pull request"


def first_line(body: str) -> str:
    lines = [line.strip() for line in (body or "").splitlines() if line.strip()]
    return lines[0] if lines else ""


def is_standing_down(comment: dict) -> bool:
    return bool(FIRST_LINE.match(first_line(comment.get("body", "")))) and ghrest.can_push(comment)


def is_own(comment: dict) -> bool:
    """A comment the orchestrator posts itself: a `Brief` or a `Context update`."""
    return bool(OWN.match(first_line(comment.get("body", ""))))


def issue_of(comment: dict) -> int | None:
    """The issue number a REST issue comment belongs to, from its `issue_url`."""
    found = re.search(r"/issues/(\d+)$", comment.get("issue_url", ""))
    return int(found.group(1)) if found else None


def line(comment: dict, number: int, label: str = "STANDING DOWN") -> str:
    who = (comment.get("user") or {}).get("login", "?")
    return f"{label} #{number} ({who}): {first_line(comment.get('body', ''))[:200]} {comment.get('html_url', '')}".rstrip()


class Poll:
    """Each call: the lines for coders' comments not said yet.

    `get(path)` is `gh api` (parsed JSON, or `None` on a failure);
    `batch(number)` what [`in_batch`] answers for an issue ([`BRIEFED`],
    [`CLOSED_BY_PULL`], falsy when not the batch's), `None` when GitHub did
    not say. A comment is marked said only once that answer is real: a failed
    page is asked again next poll, never read as "not ours" (#1876's review)."""

    def __init__(self, get: Callable[[str], object], since: datetime, batch: Callable[[int], object] | None = None):
        self.get, self.seen = get, set()
        self.since = (since - timedelta(minutes=LOOKBACK_MINUTES)).astimezone(timezone.utc)
        self.batch = batch or (lambda number: in_batch(get, number))

    def __call__(self) -> list[str]:
        stamp = self.since.strftime("%Y-%m-%dT%H:%M:%SZ")
        try:
            listed = ghrest.pages(self.get, f"repos/{ghrest.slug()}/issues/comments?since={stamp}") or []
        except SystemExit:  # no repository to ask: the log's stream goes on without the poll
            return []
        said, answers = [], {}  # one question per issue per poll
        for comment in listed:
            number = issue_of(comment)
            if comment.get("id") in self.seen or number is None or is_own(comment) or not ghrest.can_push(comment):
                continue
            if number not in answers:
                try:
                    answers[number] = self.batch(number)
                except SystemExit:
                    answers[number] = None
            ours = answers[number]
            if ours is None:
                continue
            self.seen.add(comment.get("id"))
            if is_standing_down(comment) and ours:
                said.append(line(comment, number))
            elif ours == BRIEFED:
                said.append(line(comment, number, "COMMENT"))
        return said


def in_batch(get: Callable[[str], object], number: int) -> str | bool | None:
    """[`BRIEFED`] when issue `number` carries a trusted `Brief`,
    [`CLOSED_BY_PULL`] when only an open pull request closes it, `False` when
    neither; `None` when a page GitHub was asked for did not come."""
    comments = ghrest.pages(get, f"repos/{ghrest.slug()}/issues/{number}/comments")
    if comments is None:
        return None
    if any(brief.is_brief(c.get("body", "")) and ghrest.can_push(c) for c in comments):
        return BRIEFED
    pulls = ghrest.pages(get, f"repos/{ghrest.slug()}/pulls?state=open")
    if pulls is None:
        return None
    closed = any(int(n) == number for p in pulls for n in ghrest.CLOSING.findall(p.get("body") or ""))
    return CLOSED_BY_PULL if closed else False


# How long one `gh api` call may take. The poll runs in the stream's one
# process, so a call that hangs would stop the log tail and the liveness check
# with it (#1876's review); a timeout is a failed poll, asked again next time.
GH_TIMEOUT_SECONDS = 30


def gh_get(path: str) -> object:
    """`gh api path`, parsed; `None` when it failed or timed out (the next poll tries again)."""
    try:
        done = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False,
                              timeout=GH_TIMEOUT_SECONDS)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if done.returncode != 0:
        return None
    try:
        return json.loads(done.stdout)
    except ValueError:
        return None
