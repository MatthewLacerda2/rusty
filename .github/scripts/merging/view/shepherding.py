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
- **It works out what a record owes from GitHub alone** ([`Shepherding.poll`]),
  on its first poll and on every poll after, however old the record: the
  operator's machine loses power without warning and the network may stay
  down after it, so nothing rests on what a stream saw while it ran (the
  #2650 review: a round posted while the watch was down was never relaunched).
  Each poll lists the open pull requests and reads the thread of one whose
  `updated_at` moved ([`Shepherding.records`]); a pull request whose newest
  trusted record says `round` or `hold` on its head now ([`newest`]) owes:
  - **a `round`, in draft**: its coder relaunched on the record's `Round brief:`
    comment (`make coder ISSUE=M`, since the claude.ai login that launches
    coders lives on this machine, not in the shepherd's cloud session), unless
    a coder routine for the issue on that brief or made after the record, or a
    place in the coders' waiting line, shows it was ([`relaunched`]): **the
    routine's existence is the memory**, so two polls, or two streams, launch
    once; a routines listing or a waiting line not read waits for the next
    poll. One line says the relaunch; a failed one is said once per stream and
    tried again on each poll;
  - **a `hold`**, or a `round` with no `Round brief:` line: one line ([`told`]),
    the record's `Sent up:` lines (or else its first line), said once: the ids
    told are kept in `quiet.json` ([`KEEP_SEEN`] of them), with the waiting
    starts, so a restart says none twice; on a machine's first stream every
    standing `hold` is said once.
  A `queue` is said by nobody: the watch takes it.

`--no-shepherd` on `make queue-quiet` turns all of this off in one word: the
ready lines are said as before and no record is acted on.
"""

from __future__ import annotations

import json
import re
from typing import Callable

from dispatch import running, stage
from merging.verdict import reviewed, standing
from merging.watch import board

PREFIX = "QUIET:"

#: `make shepherd`'s exit at its cap: ask again later.
AT_CAP = 3

#: How many told record ids are kept.
KEEP_SEEN = 200

ROUND_BRIEF = stage.ROUND_BRIEF
SENT_UP = re.compile(r"^[ \t]*Sent up:[ \t]*(\S.*?)[ \t\r]*$", re.M)


def wants(pull: dict | None, comments: list[dict] | None) -> bool:
    """Whether a pull request that turned ready is the shepherd's to start on."""
    if pull is None or comments is None or (pull.get("standing") or {}).get("unread"):
        return False
    if pull.get("state") not in (None, "OPEN") or pull.get("isDraft"):
        return False
    if board.queued(pull) or board.HOLD_LABEL in board.names(pull.get("labels")):
        return False  # the `queue` label's override, or a hold someone put on by hand
    found = pull.get("standing") or {}
    if found.get("said"):
        return found.get("current") is False and found.get("said") != standing.HOLD and not found.get("owed")
    return not reviewed.exempt(pull, reviewed.by_bot(pull)) and not reviewed.decided(comments)


def newest(pull: dict, comments: list[dict]) -> tuple[str, dict] | None:
    """`(word, record)` when `pull`'s newest trusted record ends in a `round` or a `hold`
    on its head now (REST `head.sha`); `None` for a `queue`, an older head or no record."""
    found = standing.records(comments)
    verdict = standing.VERDICT.findall((found[-1].get("body") or "") if found else "")
    if not verdict:
        return None
    word, sha = verdict[-1][0].lower(), verdict[-1][1].lower()
    if word == standing.QUEUE or not str((pull.get("head") or {}).get("sha") or "").startswith(sha):
        return None
    return word, found[-1]


def relaunched(issue: int, record: dict, routines: list[dict], waiting: set[int]) -> bool:
    """Whether #`issue`'s coder was relaunched for `record`'s round: waiting for a slot, or
    a coder routine on the brief it names, or made after the record was posted."""
    if issue in waiting:
        return True
    posted = running.when(record.get("created_at"))
    brief = ROUND_BRIEF.search(record.get("body") or "")
    for made in routines:
        at = running.when(made.get("created_at"))
        if running.issue(made) == issue and (
                (brief and brief.group(1) in json.dumps(made))
                or (at and posted and at > posted)):
            return True
    return False


def sha_of(record: dict) -> str:
    return standing.VERDICT.findall(record.get("body") or "")[-1][1][:7]


def told(number: int, word: str, record: dict) -> str:
    sha, url = sha_of(record), record.get("html_url", "")
    if word == standing.ROUND:
        return f"{PREFIX} #{number}: round sent on {sha} with no `Round brief:` line: relaunch its coder by hand: {url}"
    why = "; ".join(SENT_UP.findall(record.get("body") or "")) or reviewed.first_line(record.get("body") or "").strip()
    return f"{PREFIX} #{number}: held on {sha}: {why[:300]} {url}"


class Shepherding:
    """`fx` carries `shepherd(number)` (`(status, line)` of `make shepherd`),
    `ready(number)` (`quiet.Quiet`'s read), `pulls()` (the open pull requests,
    REST, `None` on a failure), `thread(number)` (a pull request's comments,
    `None` on a failure), `coders()` (the routines listed, `None` on a failure),
    `waiting()` (the issues in the coders' waiting line, `None` unread) and
    `coder(issue)` (lines of what failed)."""

    def __init__(self, fx, kept: dict):
        self.fx = fx
        self.waiting = sorted({n for n in kept.get("waiting") or [] if isinstance(n, int)})
        self.seen = [i for i in kept.get("records") or [] if isinstance(i, int)]
        self.read: dict[tuple, tuple[str, dict] | None] = {}  # (number, updated_at) -> `newest`
        self.failed: set[int] = set()  # records whose relaunch failed, said once per stream

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

    def records(self) -> list[tuple[dict, str, dict]] | None:
        """Each open pull request whose newest record is a `round` or `hold` on its head,
        re-reading a thread only when the pull request's `updated_at` moved."""
        listed = self.fx.pulls()
        if listed is None:
            return None
        found, read = [], {}
        for pull in listed:
            key = (pull.get("number"), pull.get("updated_at"))
            if key not in self.read:
                comments = self.fx.thread(pull.get("number"))
                if comments is None:
                    continue
                self.read[key] = newest(pull, comments)
            read[key] = self.read[key]
            if read[key]:
                found.append((pull, *read[key]))
        self.read = read
        return found

    def poll(self) -> list[str]:
        """What GitHub alone says is owed, on a start and each poll, however old the record:
        a `round` in draft whose coder was never relaunched, and a `hold` not told yet."""
        said, routines, waiting = [], None, None
        for pull, word, record in self.records() or []:
            number, ident = pull.get("number"), record.get("id")
            issue = ROUND_BRIEF.search(record.get("body") or "")
            if word == standing.HOLD or not issue:
                if word == standing.ROUND and not pull.get("draft"):
                    continue
                if isinstance(ident, int) and ident not in self.seen:
                    self.seen.append(ident)
                    said.append(told(number, word, record))
                continue
            if not pull.get("draft"):
                continue
            routines = self.fx.coders() if routines is None else routines
            waiting = self.fx.waiting() if waiting is None else waiting
            issue = int(issue.group(2))
            if routines is None or waiting is None or relaunched(issue, record, routines, waiting):
                continue  # a doubt waits for the next poll; a relaunch is never repeated
            failed = self.fx.coder(issue)
            if not failed:
                said.append(f"{PREFIX} #{number}: round sent on {sha_of(record)},"
                            f" its coder relaunched on #{issue}: {record.get('html_url', '')}")
                routines = None  # listed again, so a second round on the same issue sees this launch
            elif ident not in self.failed:
                self.failed.add(ident)
                said += failed
        return said


def relaunch(run: Callable[[list[str]], tuple[int, str]], issue: int) -> list[str]:
    """`make coder ISSUE=issue`, and what failed."""
    status, why = run(["python3", ".github/scripts/coder.py", str(issue)])
    return [f"{PREFIX} relaunching #{issue}'s coder for its round failed (exit {status}): {why}"] if status else []
