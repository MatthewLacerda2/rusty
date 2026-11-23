"""The quiet watch's part in the shepherd's loop (#2629).

`make queue-quiet` used to say "#N is ready with no review decision" and wait
for the orchestrator to run the review loop. Now it starts the shepherd
(`make shepherd PR=N`, `dispatch/shepherd.py`) itself, and tells the
orchestrator only the verdicts that ask something of it:

- **It starts one on each pull request that is ready with no standing
  verdict on its head** ([`wants`]): no review record and not exempt, or a
  verdict whose head has commits of its own since (a round came back). Each
  poll finds them from GitHub alone ([`Shepherding.readies`]), so one that
  turned ready while the watch was down, or while a read failed, is started
  on the next poll; a "turned ready" line only starts it sooner. A read that
  leaves the answer open ([`unsure`]: the comments unread, or a verdict whose
  compare with the head failed) is never taken for "owes nothing": it is read
  again on the next poll. A start is
  remembered by its head in `quiet.json` (`heads`), so a restart or a second
  poll never starts two on one head, however many routines were made since. A
  pull request that is reviewed, held, owes a `Needs:`, or needs no record is
  left to `quiet.ready_line` on its log line, as before, and is not read
  again until it moves (`updated_at`). A start the shepherd's cap refuses
  (exit 3) waits in [`Shepherding.waiting`] and is asked again on each drain
  pass; one refused for another reason (its shepherd still in flight, a stop)
  is silent; one that failed (exit 2) is said, with the ready line it stood in
  for. **A limit, written down rather than built:** a shepherd that started
  and ended without posting a record is not started again on that head, and
  the watch says nothing of it. A ready pull request that sits with no record
  is the orchestrator's to notice: it runs `make shepherd PR=N` again, or the
  review by hand.
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
    poll. A relaunch, or a routine or waiting place that shows one, is also
    kept by the record's id in `quiet.json` (`relaunched`), so "once" does not
    rest on the routines API listing only the account's newest 20. One line
    says the relaunch; a failed one is said once per stream and tried again on
    each poll;
  - **a `hold`**, or a `round` with no `Round brief:` line: one line ([`told`]),
    the record's `Sent up:` lines (or else its first line), said once: the ids
    told are kept in `quiet.json` ([`KEEP_SEEN`] of them), with the waiting
    starts, so a restart says none twice; on a machine's first stream every
    standing `hold` is said once.
  A `queue` is said by nobody: the watch takes it.

`--no-shepherd` on `make queue-quiet` turns all of this off in one word: no
shepherd starts and no record is acted on; a pull request the shepherd would
have started on gets its ready line instead, once per head, found the same way.
Those heads are kept apart from the starts (`lined`, not `heads`), so a later
watch without the flag still starts a shepherd on a head that only had its line.
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


def unsure(pull: dict | None, comments: list[dict] | None) -> bool:
    """Whether a read leaves open what the pull request owes: GitHub did not answer, its
    comments were not read (`unread`), or it says a verdict whose head could not be
    compared (`current` `None`). Such a pull request waits for the next poll."""
    if pull is None or comments is None:
        return True
    found = pull.get("standing") or {}
    return bool(found.get("unread")) or (bool(found.get("said")) and found.get("current") is None)


def wants(pull: dict | None, comments: list[dict] | None) -> bool:
    """Whether a pull request that turned ready is the shepherd's to start on."""
    if unsure(pull, comments):
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
    """`fx` carries `shepherd(number)` (`(status, line)` of `make shepherd`, `None` with
    `--no-shepherd`: `line(number, pull, comments)` is said instead, once per head),
    `ready(number)` (`quiet.Quiet`'s read), `pulls()` (the open pull requests,
    REST, `None` on a failure), `thread(number)` (a pull request's comments,
    `None` on a failure), `coders()` (the routines listed, `None` on a failure),
    `waiting()` (the issues in the coders' waiting line, `None` unread) and
    `coder(issue)` (lines of what failed)."""

    def __init__(self, fx, kept: dict, line: Callable[[int, dict, list[dict]], str | None] = lambda *_: None):
        self.fx, self.line, self.on = fx, line, fx.shepherd is not None
        self.waiting = sorted({n for n in kept.get("waiting") or [] if isinstance(n, int)})
        self.seen = [i for i in kept.get("records") or [] if isinstance(i, int)]
        self.relaunches = [i for i in kept.get("relaunched") or [] if isinstance(i, int)]
        heads = kept.get("heads") if isinstance(kept.get("heads"), dict) else {}
        self.heads = {k: v for k, v in heads.items() if isinstance(v, str)}  # str(number) -> head started on
        lined = kept.get("lined") if isinstance(kept.get("lined"), dict) else {}
        self.lined = {k: v for k, v in lined.items() if isinstance(v, str)}  # ... -> head said under --no-shepherd
        self.read: dict[tuple, tuple[str, dict] | None] = {}  # (number, updated_at) -> `newest`
        self.passed: set[tuple] = set()  # (number, updated_at) read and owing no start
        self.failed: set[int] = set()  # records whose relaunch failed, said once per stream

    def kept(self) -> dict:
        """What `quiet.json` keeps: copies, so `quiet.Quiet` comparing it with what it last
        saved sees a start or a line written in place since."""
        return {"waiting": self.waiting, "records": self.seen[-KEEP_SEEN:],
                "relaunched": self.relaunches[-KEEP_SEEN:], "heads": dict(self.heads), "lined": dict(self.lined)}

    def done(self, number, head: str) -> bool:
        """Whether `head` needs nothing more from this watch: a shepherd was started on it,
        or, with `--no-shepherd`, it had its ready line. A line is not a start, so a watch
        with the shepherd on still starts one on a head that only had its line."""
        return self.heads.get(str(number)) == head or (not self.on and self.lined.get(str(number)) == head)

    def begin(self, number: int, pull: dict, comments: list[dict]) -> list[str]:
        """Act on a pull request [`wants`] says is the shepherd's, once per head: start its
        shepherd, or with `--no-shepherd` say its ready line."""
        head = str(pull.get("headRefOid") or "")
        if self.done(number, head):
            return []
        if not self.on:
            self.lined[str(number)] = head
            said = self.line(number, pull, comments)
            return [said] if said else []
        return self.start(number, head)

    def start(self, number: int, head: str | None = None) -> list[str]:
        """Start #`number`'s shepherd; the lines to say (none when it started or was refused).
        A start is remembered by its head (`heads`, in `quiet.json`), so none is asked twice."""
        status, line = self.fx.shepherd(number)
        if status == AT_CAP:
            self.waiting = sorted({*self.waiting, number})
            return []
        self.waiting = [n for n in self.waiting if n != number]
        if status == 0 and head:
            self.heads[str(number)] = head
        if status in (0, 1):
            return []
        return [f"{PREFIX} #{number} turned ready, and its shepherd could not start (exit {status}): {line}."
                f" Run `make shepherd PR={number}`, or review it by hand."]

    def readies(self, listed: list[dict]) -> list[str]:
        """Each listed pull request that is ready with no standing verdict on its head and no
        start on that head yet: read, and [`begin`] on. A read that fails waits for the next
        poll; one that owes nothing is not read again until the pull request moves."""
        said, open_now, passed = [], set(), set()
        for pull in listed:
            number, head = pull.get("number"), str((pull.get("head") or {}).get("sha") or "")
            key = (number, pull.get("updated_at"))
            open_now.add(str(number))
            if pull.get("draft") or number in self.waiting or self.done(number, head):
                continue
            if key in self.passed:
                passed.add(key)
                continue
            got, comments = self.fx.ready(number)
            if unsure(got, comments):
                continue  # not "owes nothing": read again on the next poll
            if wants(got, comments):
                said += self.begin(number, got, comments)
            if not self.done(number, str(got.get("headRefOid") or "")) and number not in self.waiting:
                passed.add(key)  # owes nothing, was refused, or failed and was said: until it moves
        self.passed = passed
        self.heads = {k: v for k, v in self.heads.items() if k in open_now}
        self.lined = {k: v for k, v in self.lined.items() if k in open_now}
        return said

    def retry(self) -> list[str]:
        said = []
        for number in list(self.waiting):
            pull, comments = self.fx.ready(number)
            if unsure(pull, comments):
                continue
            if not wants(pull, comments):
                self.waiting = [n for n in self.waiting if n != number]
                continue
            said += self.begin(number, pull, comments)
        return said

    def records(self, listed: list[dict]) -> list[tuple[dict, str, dict]]:
        """Each open pull request whose newest record is a `round` or `hold` on its head,
        re-reading a thread only when the pull request's `updated_at` moved."""
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
        listed = self.fx.pulls()
        if listed is None:
            return []
        said = self.readies(listed)
        if not self.on:
            return said
        routines = waiting = None
        for pull, word, record in self.records(listed):
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
            if ident in self.relaunches:
                continue  # relaunched once: kept in `quiet.json`, never repeated
            routines = self.fx.coders() if routines is None else routines
            waiting = self.fx.waiting() if waiting is None else waiting
            issue = int(issue.group(2))
            if routines is None or waiting is None:
                continue  # a doubt waits for the next poll
            if relaunched(issue, record, routines, waiting):
                self.relaunches.append(ident)
                continue
            failed = self.fx.coder(issue)
            if not failed:
                self.relaunches.append(ident)
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
