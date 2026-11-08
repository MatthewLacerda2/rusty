"""Coders' *Standing down* comments, for `make queue-events` (#1876).

A cloud coder that cannot start (an open blocker, a decision it cannot make)
says so on its issue, in a comment whose first line begins `Standing down`
(`issue-batch/cloud-brief.md`), not on a pull request, so the watch's log never
sees it. This is the light poll beside the log: the repository's issue comments
updated since the stream began, less [`LOOKBACK_MINUTES`], kept when the first
line says *Standing down*, its author can push (`ghrest.can_push`: the
repository is public), and its issue is the batch's (a `Brief` on it, or an
open pull request that closes it). Each comment is said once per stream.

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


def first_line(body: str) -> str:
    lines = [line.strip() for line in (body or "").splitlines() if line.strip()]
    return lines[0] if lines else ""


def is_standing_down(comment: dict) -> bool:
    return bool(FIRST_LINE.match(first_line(comment.get("body", "")))) and ghrest.can_push(comment)


def issue_of(comment: dict) -> int | None:
    """The issue number a REST issue comment belongs to, from its `issue_url`."""
    found = re.search(r"/issues/(\d+)$", comment.get("issue_url", ""))
    return int(found.group(1)) if found else None


def line(comment: dict, number: int) -> str:
    who = (comment.get("user") or {}).get("login", "?")
    return f"STANDING DOWN #{number} ({who}): {first_line(comment.get('body', ''))[:200]} {comment.get('html_url', '')}".rstrip()


class Poll:
    """Each call: the lines for *Standing down* comments not said yet.

    `get(path)` is `gh api` (parsed JSON, or `None` on a failure);
    `batch(number)` whether an issue is the batch's ([`in_batch`]), `None` when
    GitHub did not say. A comment is marked said only once that answer is
    real: a failed page is asked again next poll, never read as "not ours"
    (#1876's review)."""

    def __init__(self, get: Callable[[str], object], since: datetime, batch: Callable[[int], bool] | None = None):
        self.get, self.seen = get, set()
        self.since = (since - timedelta(minutes=LOOKBACK_MINUTES)).astimezone(timezone.utc)
        self.batch = batch or (lambda number: in_batch(get, number))

    def __call__(self) -> list[str]:
        stamp = self.since.strftime("%Y-%m-%dT%H:%M:%SZ")
        try:
            listed = ghrest.pages(self.get, f"repos/{ghrest.slug()}/issues/comments?since={stamp}") or []
        except SystemExit:  # no repository to ask: the log's stream goes on without the poll
            return []
        said = []
        for comment in listed:
            number = issue_of(comment)
            if comment.get("id") in self.seen or number is None or not is_standing_down(comment):
                continue
            try:
                ours = self.batch(number)
            except SystemExit:
                ours = None
            if ours is None:
                continue
            self.seen.add(comment.get("id"))
            if ours:
                said.append(line(comment, number))
        return said


def in_batch(get: Callable[[str], object], number: int) -> bool | None:
    """Whether issue `number` carries a trusted `Brief`, or an open pull request
    closes it; `None` when a page GitHub was asked for did not come."""
    comments = ghrest.pages(get, f"repos/{ghrest.slug()}/issues/{number}/comments")
    if comments is None:
        return None
    if any(brief.is_brief(c.get("body", "")) and ghrest.can_push(c) for c in comments):
        return True
    pulls = ghrest.pages(get, f"repos/{ghrest.slug()}/pulls?state=open")
    if pulls is None:
        return None
    return any(int(n) == number for p in pulls for n in ghrest.CLOSING.findall(p.get("body") or ""))


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
