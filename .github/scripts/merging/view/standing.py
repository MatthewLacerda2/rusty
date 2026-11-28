"""Coders' comments on their batch issues, for `make queue-events` (#1876, #2136).

A cloud coder that cannot start (an open blocker), hands its work back (a
decision it cannot make) or notices something after readying says so on its
issue, not on a pull request, so the watch's log never sees it, and a draft
that stays a draft raises no event either. This is the light poll beside the
log: the repository's issue comments updated since the stream began, less
[`LOOKBACK_MINUTES`], kept when the comment is on an issue (not a pull
request), its author can push (`ghrest.can_push`: the repository is public)
and the issue is the batch's ([`in_batch`]). Each comment is said once per
stream, and once across cloud fires: `make queue-quiet` keeps the newest poll's
time and the ids said in `quiet.json` ([`Poll.kept`]), which travels with the
orchestrator lock, and the next fire reads from that poll on ([`Poll.resume`]),
so a comment posted after one fire's poll is the next fire's.

Two kinds pass:

- **A stand-down** (first line `Standing down`, the wording `cloud-brief.md`
  prefers): `STANDING DOWN #N …`, on an issue with a `Brief` (open or
  closed) or an open pull request that closes it.
- **Any other comment** on an open issue with a trusted `Brief`: `COMMENT
  #N …`. A coder's comment on its batch issue is something the orchestrator
  should read (a hand-back, a note after readying), so whatever its wording it
  is an event. An issue with only a pull request (no `Brief`) is not a
  coder's, and a closed one is no longer the batch's, so their discussion
  stays out.

The orchestrator comments as the same account as every coder, so its own
comments are told apart by their first line, not their author ([`OWN`]): its
`Brief`s and its notes, headed `Context update`, are skipped. A note of its own
under any other heading passes: a spurious line costs a glance, a missed
hand-back hours.

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

# How many said comment ids [`Poll.kept`] keeps: far more than a look back's worth.
KEEP_SEEN = 300

FIRST_LINE = re.compile(r"^\W*Standing down\b", re.IGNORECASE)

# The orchestrator's own comments on a batch issue, by first line.
OWN = re.compile(r"^\W*(Brief|Context update)\b", re.IGNORECASE)

# What [`in_batch`] answers: a trusted `Brief` on an open issue, on a closed
# one, or only an open pull request closing it.
BRIEFED, BRIEFED_CLOSED, CLOSED_BY_PULL = "briefed", "briefed, closed", "closed by a pull request"


def first_line(body: str) -> str:
    lines = [line.strip() for line in (body or "").splitlines() if line.strip()]
    return lines[0] if lines else ""


def is_standing_down(comment: dict) -> bool:
    return bool(FIRST_LINE.match(first_line(comment.get("body", ""))))


def is_own(comment: dict) -> bool:
    """A comment the orchestrator posts itself: a `Brief` or a `Context update` note."""
    return bool(OWN.match(first_line(comment.get("body", ""))))


def issue_of(comment: dict) -> int | None:
    """The issue number a REST issue comment belongs to, from its `issue_url`;
    `None` for a pull request's conversation comment, which the same list
    returns (its `html_url` is a `/pull/` one)."""
    if "/pull/" in comment.get("html_url", ""):
        return None
    found = re.search(r"/issues/(\d+)$", comment.get("issue_url", ""))
    return int(found.group(1)) if found else None


def line(comment: dict, number: int, label: str = "STANDING DOWN") -> str:
    who = (comment.get("user") or {}).get("login", "?")
    return f"{label} #{number} ({who}): {first_line(comment.get('body', ''))[:200]} {comment.get('html_url', '')}".rstrip()


class Poll:
    """Each call: the lines for coders' comments not said yet.

    `get(path)` is `gh api` (parsed JSON, or `None` on a failure);
    `batch(number, pulls)` what [`in_batch`] answers for an issue, `pulls()`
    the open pull requests, read at most once per poll. `None` is "GitHub did
    not say": a comment is marked said only once that answer is real, so a
    failed page is asked again next poll, never read as "not ours" (#1876's
    review)."""

    def __init__(self, get: Callable[[str], object], since: datetime,
                 batch: Callable[[int, Callable[[], list | None]], object] | None = None,
                 clock: Callable[[], datetime] = lambda: datetime.now(timezone.utc)):
        self.get, self.seen, self.clock = get, set(), clock
        self.since = (since - timedelta(minutes=LOOKBACK_MINUTES)).astimezone(timezone.utc)
        self.batch = batch or (lambda number, pulls: in_batch(get, number, pulls))
        self.polled: datetime | None = None  # when the newest list that came was asked for

    def resume(self, kept: object) -> None:
        """Carry on from [`kept`]'s record (`quiet.json`'s, so a fresh cloud fire reads from
        the last fire's last poll, not from when its lease ended): read since that poll, less
        the look back, and never say again what it said. A record that is not one is ignored."""
        if not isinstance(kept, dict) or not isinstance(kept.get("polled"), str):
            return
        try:
            polled = datetime.fromisoformat(kept["polled"].replace("Z", "+00:00"))
        except ValueError:
            return
        self.since = min(self.since, polled - timedelta(minutes=LOOKBACK_MINUTES))
        self.polled = polled
        self.seen |= {i for i in kept.get("seen") or [] if isinstance(i, int)}

    def kept(self) -> dict | None:
        """What [`resume`] takes back: the newest poll's time and the newest [`KEEP_SEEN`] said ids."""
        if self.polled is None:
            return None
        seen = sorted(i for i in self.seen if isinstance(i, int))[-KEEP_SEEN:]
        return {"polled": self.polled.strftime("%Y-%m-%dT%H:%M:%SZ"), "seen": seen}

    def __call__(self) -> list[str]:
        stamp = self.since.strftime("%Y-%m-%dT%H:%M:%SZ")
        asked = self.clock()
        try:
            listed = ghrest.pages(self.get, f"repos/{ghrest.slug()}/issues/comments?since={stamp}")
        except SystemExit:  # no repository to ask: the log's stream goes on without the poll
            return []
        if listed is None:  # GitHub did not answer: the next poll asks for the same window
            return []
        said, answers, pulls = [], {}, once(lambda: ghrest.pages(self.get, f"repos/{ghrest.slug()}/pulls?state=open"))
        for comment in listed:
            number = issue_of(comment)
            if comment.get("id") in self.seen or number is None or is_own(comment) or not ghrest.can_push(comment):
                continue
            if number not in answers:  # one question per issue per poll
                try:
                    answers[number] = self.batch(number, pulls)
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
        if None not in answers.values():  # a comment left unread keeps the window where it is
            self.polled = asked
        return said


def once(read: Callable[[], list | None]) -> Callable[[], list | None]:
    """`read`, called at most once; a failed read (`None`) is not asked again in the same poll."""
    kept: list = []

    def call():
        if not kept:
            kept.append(read())
        return kept[0]
    return call


def in_batch(get: Callable[[str], object], number: int, pulls: Callable[[], list | None]) -> str | bool | None:
    """[`BRIEFED`] when open issue `number` carries a trusted `Brief`,
    [`BRIEFED_CLOSED`] when the issue is closed, [`CLOSED_BY_PULL`] when only
    an open pull request (`pulls()`) closes it, `False` when neither; `None`
    when a page GitHub was asked for did not come."""
    comments = ghrest.pages(get, f"repos/{ghrest.slug()}/issues/{number}/comments")
    if comments is None:
        return None
    if any(brief.is_brief(c.get("body", "")) and ghrest.can_push(c) for c in comments):
        issue = get(f"repos/{ghrest.slug()}/issues/{number}")
        if not isinstance(issue, dict):
            return None
        return BRIEFED if issue.get("state") == "open" else BRIEFED_CLOSED
    listed = pulls()
    if listed is None:
        return None
    closed = any(int(n) == number for p in listed for n in ghrest.CLOSING.findall(p.get("body") or ""))
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
