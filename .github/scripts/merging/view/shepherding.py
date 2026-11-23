"""The quiet watch's part in the shepherd's loop (#2629).

`make queue-quiet` used to say "#N is ready with no review decision" and wait
for the orchestrator to run the review loop. Now it starts the shepherd
(`make shepherd PR=N`, `dispatch/shepherd.py`) itself, and tells the
orchestrator only the verdicts that ask something of it:

- **It starts one when a pull request turns ready with no standing verdict**
  ([`wants`]): no review record and not exempt, or a verdict whose head has
  commits of its own since (a round came back). A pull request that is
  reviewed, held, owes a `Needs:`, or needs no record is left to
  `quiet.ready_line`, as before. A start the shepherd's cap refuses (exit 3)
  waits in [`Shepherding.waiting`] and is asked again on each drain pass; one
  refused for another reason (its shepherd still in flight, a stop) is silent;
  one that failed (exit 2) is said, with the ready line it stood in for.
- **It says one line per record that is not `queue`** ([`told`]), read from the
  repository's comments since the stream began, less the same look back as the
  coders' poll: a `round` sent (and the coder relaunched on its
  `Round brief:` comment, `make coder ISSUE=M`, since the claude.ai login that
  launches coders lives on this machine, not in the shepherd's cloud session),
  or a `hold` and why (the record's `Sent up:` lines, or else its first line).
  A `queue` is said by nobody: the watch takes it.
- **Each record is acted on once**: the ids told are kept in `quiet.json`
  ([`KEEP_SEEN`] of them), with the waiting starts, so a restart that reads the
  same comments again relaunches no coder twice. With no ids kept yet (the
  first stream on a machine), the first poll only marks what it reads: a
  record posted before the watch knew of it is the orchestrator's, as before.

`--no-shepherd` on `make queue-quiet` turns all of this off in one word: the
ready lines are said as before and no record is acted on.
"""

from __future__ import annotations

import re
from typing import Callable

import ghrest
from merging.verdict import reviewed, standing

PREFIX = "QUIET:"

#: `make shepherd`'s exit at its cap: ask again later.
AT_CAP = 3

#: How many told record ids are kept.
KEEP_SEEN = 200

ROUND_BRIEF = re.compile(r"^[ \t]*Round brief:[ \t]*https://github\.com/\S+/issues/(\d+)#issuecomment-\d+", re.M)
SENT_UP = re.compile(r"^[ \t]*Sent up:[ \t]*(\S.*?)[ \t\r]*$", re.M)
PULL = re.compile(r"/pull/(\d+)")


def wants(pull: dict | None, comments: list[dict] | None) -> bool:
    """Whether a pull request that turned ready is the shepherd's to start on."""
    if pull is None or comments is None or (pull.get("standing") or {}).get("unread"):
        return False
    if pull.get("state") not in (None, "OPEN") or pull.get("isDraft"):
        return False
    if "hold" in {(label or {}).get("name") for label in pull.get("labels") or []}:
        return False
    found = pull.get("standing") or {}
    if found.get("said"):
        return found.get("current") is False and found.get("said") != standing.HOLD and not found.get("owed")
    return not reviewed.exempt(pull, False) and not reviewed.decided(comments)


def told(comment: dict) -> tuple[str, int | None] | None:
    """The line a record asks for and the issue whose coder a `round` relaunches;
    `None` for a comment that is no trusted record with a verdict, or a `queue`."""
    body = comment.get("body") or ""
    number = PULL.search(comment.get("html_url") or "")
    if not number or not ghrest.can_push(comment) or not reviewed.decides(body):
        return None
    found = standing.VERDICT.findall(body)
    if not found or found[-1][0].lower() == standing.QUEUE:
        return None
    word, sha = found[-1][0].lower(), found[-1][1][:7]
    url = comment.get("html_url", "")
    if word == standing.ROUND:
        issue = ROUND_BRIEF.search(body)
        return (f"{PREFIX} #{number.group(1)}: round sent on {sha}, its coder relaunched"
                f"{f' on #{issue.group(1)}' if issue else ' (no `Round brief:` line: relaunch it by hand)'}: {url}",
                int(issue.group(1)) if issue else None)
    why = "; ".join(SENT_UP.findall(body)) or reviewed.first_line(body).strip()
    return f"{PREFIX} #{number.group(1)}: held on {sha}: {why[:300]} {url}", None


class Shepherding:
    """`fx` carries `shepherd(number)` (`(status, line)` of `make shepherd`),
    `coder(issue)` (lines of what failed), `records()` (the comments since the
    stream began, `None` on a failure) and `ready(number)` (`quiet.Quiet`'s read)."""

    def __init__(self, fx, kept: dict):
        self.fx = fx
        self.waiting = sorted({n for n in kept.get("waiting") or [] if isinstance(n, int)})
        self.seen = [i for i in kept.get("records") or [] if isinstance(i, int)]
        # With nothing kept yet (the first stream ever), the first poll only marks what it reads.
        self.primed = isinstance(kept.get("records"), list)

    def kept(self) -> dict:
        return {"waiting": self.waiting, "records": self.seen[-KEEP_SEEN:]}

    def start(self, number: int) -> list[str]:
        """Start #`number`'s shepherd; the lines to say (none when it started or was refused)."""
        status, line = self.fx.shepherd(number)
        if status == AT_CAP:
            self.waiting = sorted({*self.waiting, number})
            return []
        self.waiting = [n for n in self.waiting if n != number]
        if status in (0, 1):
            return []
        return [f"{PREFIX} #{number} turned ready, and its shepherd could not start (exit {status}): {line}."
                f" Run `make shepherd PR={number}`, or review it by hand."]

    def retry(self) -> list[str]:
        said = []
        for number in list(self.waiting):
            pull, comments = self.fx.ready(number)
            if pull is None or comments is None:
                continue
            if not wants(pull, comments):
                self.waiting = [n for n in self.waiting if n != number]
                continue
            said += self.start(number)
        return said

    def poll(self) -> list[str]:
        listed = self.fx.records()
        said = []
        for comment in listed or []:
            ident = comment.get("id")
            if not isinstance(ident, int) or ident in self.seen or (found := told(comment)) is None:
                continue
            self.seen.append(ident)
            if not self.primed:
                continue
            line, issue = found
            said.append(line)
            if issue is not None:
                said += self.fx.coder(issue)
        self.primed = self.primed or listed is not None
        return said


def relaunch(run: Callable[[list[str]], tuple[int, str]], issue: int) -> list[str]:
    """`make coder ISSUE=issue`, and what failed."""
    status, why = run(["python3", ".github/scripts/coder.py", str(issue)])
    return [f"{PREFIX} relaunching #{issue}'s coder for its round failed (exit {status}): {why}"] if status else []
