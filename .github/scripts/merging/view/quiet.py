"""`make queue-quiet`: the batch's routine events handled, only what needs someone said (#2628).

`make queue-events` passes every event of a batch to the orchestrator's
`Monitor`, and each one wakes a session that re-reads its whole context to
write two lines: a merge ("fast-forward, `make reclaim`"), a pull request
opening as a draft, a watch restarting. Whatever a script can do, a script
does, so this follows the same log ([`events.Tail`]) and:

- **does the routine itself**: on each merge ([`events.MERGED`]) it
  fast-forwards the main checkout to `origin/main` and runs `make reclaim`
  (`after_merge`), saying only what failed; it runs the coders' drain pass
  (`make coders`, `drain`) every [`DRAIN_SECONDS`], as the orchestrator's
  loop did, saying a failing pass once, after [`DRAIN_FAILS`] in a row;
- **starts the shepherd** (`make shepherd PR=N`, [`shepherding`], #2629) on
  a pull request turning ready with no review decision or past its verdict's
  head, and says one line per verdict that is not `queue` (a round sent, and
  its coder relaunched; a hold and why); `--no-shepherd` turns it off, and the
  ready lines below are said instead;
- **says only what someone must act on**, one line each: a pull request
  turning ready with no review decision, reviewed with nothing that queues
  it, or on a head with commits of its own past its standing verdict (a round
  came back; it stays quiet only while the head is the verdict's, or only
  merges followed) ([`ready_line`]); a hand-back, a held Dependabot bump, a state file
  not written, a lost mutation request, a supervisor or watch that stopped
  (the lines [`events.KEEP`] passes, less the board's own); a new `bug` issue
  ([`new_bugs`]); a red gating run on `main` ([`red_main`]); and a coder's
  comment on its issue ([`standing.Poll`]). A draft opening, a merge, a
  pull request closing or going back to draft are said by nobody;
- **after a restart acts on the log's last 30 lines only** ([`events.Tail`]'s
  replay): a merge, a ready flip or a hand-back older than that is not acted
  on again, nor said; the merge's fast-forward is caught up by the next
  merge's, since each moves the main checkout to `origin/main`, not to that
  merge;
- **holds nothing a reboot loses**: what it has told of bugs and of `main`'s
  runs is kept in [`STATE`] beside the queue's other files
  (`merging/state.py`), the rest is read again from GitHub and the log, and a
  merge the 30 lines replay is fast-forwarded again, which is a no-op. Like
  `make queue-events` it ends with status 1 when the supervisor is not
  running, which is itself the line someone must act on.
"""

from __future__ import annotations

import re
from typing import Callable

from merging import state
from merging.verdict import reviewed, runlist
from merging.view import events, shepherding
from merging.watch import board

# The file beside the hand-back memory that keeps what has been told.
STATE = "quiet.json"

# How often the drain pass runs: the orchestrator's loop ran `make coders` once a minute.
DRAIN_SECONDS = 60

# How many passes in a row must fail before it is said: one fails on any GitHub or claude.ai blip.
DRAIN_FAILS = 3

# The shas of red `main` runs told, newest last, at most this many kept.
KEEP_RED = 20

# Every line this says begins so: a `Monitor` (or a reader) tells them from the log's.
PREFIX = "QUIET:"

READY = re.compile(r"watch: #(\d+) (?:turned ready|opened as ready|is ready and unqueued)")
SILENT = re.compile(r"watch: #\d+ (?:back to draft|closed|merged while no watch listed it)")

# The conclusions of a completed run that leave `main` red.
RED = ("failure", "timed_out", "startup_failure")


def ready_line(number: int, pull: dict | None, comments: list[dict] | None) -> str | None:
    """What a pull request turning ready asks of someone, or `None`. `pull` is
    read with its standing (`queue.pulls.look_standing`); `None` for either
    read is GitHub not answering."""
    if pull is None or comments is None or (pull.get("standing") or {}).get("unread"):
        return f"{PREFIX} #{number} turned ready; GitHub did not show it or its comments, so whether it is reviewed is unknown."
    if pull.get("state") not in (None, "OPEN") or pull.get("isDraft") or board.queued(pull):
        return None
    labels = board.names(pull.get("labels"))
    standing = pull.get("standing") or {}
    if board.HOLD_LABEL in labels:
        return None  # a decision someone made by hand and acts on
    if standing.get("said") and standing.get("current") is False:
        return (f"{PREFIX} #{number} is ready on {str(pull.get('headRefOid', ''))[:7]}, past its"
                f" `{standing['said']}` verdict on {standing.get('on')}: a round came back, so review it and give"
                " the new head its own `Verdict:` line.")
    if standing.get("verdict") in ("round", "hold"):
        return None  # still on the head its decision was given on: someone acts on it
    if standing.get("owed"):
        return f"{PREFIX} #{number} is reviewed and owes: {'; '.join(standing['owed'])}. Answer each with `Done: <what>`."
    if reviewed.exempt(pull, reviewed.by_bot(pull)) or reviewed.decided(comments):
        return (f"{PREFIX} #{number} is ready and needs no review or has its record, but nothing queues it:"
                " add a `Verdict: queue on <sha>` record or the `queue` label.")
    return (f"{PREFIX} #{number} is ready with no review decision: `make review-level PR={number}`, then its"
            " `Independent review` record or `No review: <why>`, with a `Verdict:` line.")


def new_bugs(issues: list[dict], told: int | None) -> tuple[list[str], int]:
    """A line per open `bug` issue newer than `told` (the highest number told);
    with nothing told yet, none, and the newest becomes the mark."""
    bugs = [i for i in issues if "pull_request" not in i]
    newest = max([i.get("number", 0) for i in bugs] + [told or 0])
    if told is None:
        return [], newest
    lines = [f"{PREFIX} new bug #{i['number']}: {i.get('title', '')} {i.get('html_url', '')}".rstrip()
             for i in sorted(bugs, key=lambda i: i.get("number", 0)) if i.get("number", 0) > told]
    return lines, newest


def red_main(runs: list[dict], told: list[str]) -> tuple[list[str], list[str]]:
    """A line per gating workflow whose newest completed run on `main` is red
    on a head not told yet; and what is told now."""
    lines, told = [], list(told)
    for workflow in runlist.WORKFLOWS:
        done = [r for r in runs if r.get("name") == workflow and r.get("status") == "completed"]
        if not done:
            continue
        newest = max(done, key=lambda r: r.get("created_at") or "")
        mark = f"{workflow}@{newest.get('head_sha', '')}"
        if newest.get("conclusion") in RED and mark not in told:
            told.append(mark)
            lines.append(f"{PREFIX} main is red: `{workflow}` concluded {newest.get('conclusion')} on"
                         f" {str(newest.get('head_sha', ''))[:7]}: {newest.get('html_url', '')}".rstrip())
    return lines, told[-KEEP_RED:]


class Quiet:
    """One stream's handling of lines and polls. `fx` carries the effects:
    `after_merge(number)` and `drain()` (lines of what failed), `ready(number)`
    (`(pull, comments)`, [`ready_line`]'s reads), `bugs()` and `main_runs()`
    (GitHub's lists, `None` on a failure), `comments()` ([`standing.Poll`]),
    `say(line)`, `path`, where [`STATE`] lives, and `shepherd`, the
    shepherd's effects (`shepherding.Shepherding`), `None` with `--no-shepherd`."""

    def __init__(self, fx):
        self.fx, self.merged, self.failing = fx, set(), 0
        kept = state.read(fx.path)
        kept = kept if isinstance(kept, dict) else {}
        self.bug = kept.get("bug") if isinstance(kept.get("bug"), int) else None
        self.red = [m for m in kept.get("red") or [] if isinstance(m, str)]
        flock = kept.get("shepherd") if isinstance(kept.get("shepherd"), dict) else {}
        shepherd = getattr(fx, "shepherd", None)
        self.flock = shepherding.Shepherding(shepherd, flock) if shepherd else None
        self.saved = self.flock and self.flock.kept()

    def line(self, line: str) -> None:
        merged = events.MERGED.search(line)
        if merged:
            number = int(merged.group(1))
            if number not in self.merged:
                self.merged.add(number)
                for said in self.fx.after_merge(number):
                    self.fx.say(said)
            return
        ready = READY.search(line)
        if ready:
            number = int(ready.group(1))
            pull, comments = self.fx.ready(number)
            if self.flock and shepherding.wants(pull, comments):
                self.tell(self.flock.start(number))
                return
            said = ready_line(number, pull, comments)
            if said:
                self.fx.say(said)
            return
        if not SILENT.search(line) and events.KEEP.search(line):
            self.fx.say(line)

    def poll(self) -> None:
        for said in self.fx.comments():
            self.fx.say(said)
        issues, runs = self.fx.bugs(), self.fx.main_runs()
        bug, red = self.bug, self.red
        if issues is not None:
            lines, bug = new_bugs(issues, self.bug)
            for said in lines:
                self.fx.say(said)
        if runs is not None:
            lines, red = red_main(runs, self.red)
            for said in lines:
                self.fx.say(said)
        if self.flock:
            self.tell(self.flock.poll())
        if (bug, red) != (self.bug, self.red):
            self.bug, self.red = bug, red
            self.save()

    def tell(self, lines: list[str]) -> None:
        """Say `lines`, and keep the shepherd's memory if it moved."""
        for said in lines:
            self.fx.say(said)
        if self.flock and self.flock.kept() != self.saved:
            self.save()

    def save(self) -> None:
        kept = {"bug": self.bug, "red": self.red}
        if self.flock:
            self.saved = kept["shepherd"] = self.flock.kept()
        failed = state.write(self.fx.path, kept)
        if failed:
            self.fx.say(f"{PREFIX} {self.fx.path} could not be written ({failed}): a restart may repeat a bug,"
                        " a red main or a round's relaunch.")

    def drain(self) -> None:
        if self.flock:
            self.tell(self.flock.retry())
        failed = self.fx.drain()
        self.failing = self.failing + 1 if failed else 0
        if self.failing == DRAIN_FAILS:
            for said in failed:
                self.fx.say(said)


def follow(quiet: Quiet, tail: events.Tail, running: Callable[[], tuple[bool, list[str]]],
           sleep: Callable[[float], None], clock: Callable[[], float]) -> int:
    """Stream until the supervisor is not running; then say why and return 1."""
    for line in tail.start():
        quiet.line(line)
    checked = polled = drained = None
    while True:
        now = clock()
        if checked is None or now - checked >= events.LIVENESS_SECONDS:
            checked = now
            alive, why = running()
            if not alive:
                for line in why:
                    quiet.fx.say(line)
                quiet.fx.say(f"{PREFIX} the supervisor is not running: start it again (`make queue-daemon`), then this.")
                return 1
        if polled is None or now - polled >= events.POLL_SECONDS:
            polled = now
            quiet.poll()
        if drained is None or now - drained >= DRAIN_SECONDS:
            drained = now
            quiet.drain()
        for line in tail.fresh():
            quiet.line(line)
        sleep(events.STEP_SECONDS)
