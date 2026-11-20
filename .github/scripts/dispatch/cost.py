"""What a coder session costs in wall-clock, and where it goes (#2452).

Every coder session starts cold, and about half of them are rounds on a branch
another session just wrote (`make rebriefs`). Whether a round should resume the
PR's own session turns on how much of a round the cold start is, and one run
log read by hand said: under 6%, the rest mostly gates. This module measures
that over every coder routine the routines API still lists.

- **One record a session** ([`measure`]), from the listing (`last_run`'s
  `session_id` and `fired_at`) and the run log (`dispatch/runlog.py`, paged back
  to its start, at most [`MOST_PAGES`] pages): seconds from the fire to `init`
  (the sandbox), from `init` to the first edit (the reading), from the first edit
  to the last `result:` (the work), the turns, and the gates' wall-clock inside
  `init`…end, split at the first edit. With no edit the whole session is
  reading, so every gate is the reading's: the four parts ([`split`]) sum to the
  total for every record, whatever it lacks.
- **First brief or round** is read from GitHub: the routine's prompt names its
  brief by URL (`routine.PROMPT`); its place among the issue's trusted briefs
  (`rebriefs.briefs_by_issue`) says which, and a round's `Round:` line its cause.
  An issue thread GitHub did not answer is not kept as unknown: the session is
  named among the skipped and read again next time (#2456's review).
- **The listing holds only the account's 20 newest routines** (`routine.py`,
  `ENVIRONMENT_FROM`), so what each run measured is kept in [`FILE`] under the
  common git directory, by session id, through `merging/state.py`; a later run
  adds to it. A session still running is not kept: it is named and read again
  next time.
- **A kept record is the finished session's.** A coder that ends its turn while
  a dispatched run works (`dispatch/running.py`'s docstring) has a `result:`
  last and reads as finished, then wakes and works on. So a record keeps the
  listing's `last_run` status and `finished_at` ([`run_end`]); a session whose
  listing moved since is read again, and its old record dropped ([`Read.stale`])
  whether or not the new read is kept (#2456's review).
- **A kept record that lacks a field or holds a non-number reads that field as
  absent** ([`number`]), the scripts' rule for state that cannot be read: a time
  it lacks makes the row `times unreadable`, never a traceback.
- **Tokens are not in the run log**, so none are counted or estimated.
- Nothing from a log's text is printed: the report is numbers, issue numbers and
  session ids, and a session id is used only when it is a plain id ([`SESSION`]).
"""

from __future__ import annotations

import re
import statistics
from dataclasses import dataclass, field
from typing import Callable

from dispatch import rebriefs, remote, runlog, running
from merging import state as files

#: The kept measurements, under the common git directory.
FILE = "coder-cost.json"
#: The most log pages read for one session: 200 events a page.
MOST_PAGES = 100
SESSION = re.compile(r"^[\w-]{1,100}$")
BRIEF = re.compile(r"#issuecomment-(\d+)")
LISTED = 20


@dataclass
class Read:
    """One pass: the records it measured, the sessions it skipped with why, and the
    kept sessions whose listing moved, whose old record is no longer theirs."""

    records: dict[str, dict] = field(default_factory=dict)
    skipped: list[str] = field(default_factory=list)
    stale: list[str] = field(default_factory=list)


def number(record: dict, key: str) -> float | None:
    """`record[key]` when it is a number, else `None`."""
    got = record.get(key)
    return got if isinstance(got, (int, float)) and not isinstance(got, bool) else None


def run_end(run: dict) -> dict:
    """What the listing says of a run's end: it moves when the session ends another turn."""
    return {"status": run.get("status"), "finished_at": run.get("finished_at")}


def prompt(found: dict) -> str:
    events = (((found.get("job_config") or {}).get("ccr") or {}).get("events")) or []
    for event in events:
        content = (((event or {}).get("data") or {}).get("message") or {}).get("content")
        if isinstance(content, str):
            return content
    return ""


def pages(session: str, ask) -> list[str] | None:
    """The run log of `session`, newest page first; `None` when it ran past [`MOST_PAGES`]."""
    got, cursor = [], None
    for _ in range(MOST_PAGES):
        arguments = {"action": "get_run_log", "session_id": session, **({"cursor": cursor} if cursor else {})}
        status, header, summary = ask(arguments)
        if status != 200:
            raise remote.RemoteError(f"get_run_log answered HTTP {status}")
        got.append(summary)
        cursor = header.get("next_cursor") if isinstance(header, dict) else None
        if not cursor:
            return got
    return None


def kind(found: dict, comments: Callable[[int], list | None]) -> tuple[str, str] | None:
    """`first` or `round` and the round's cause (`""` for a first brief), or `?` and why;
    `None` when GitHub did not answer the issue's thread."""
    match = BRIEF.search(prompt(found))
    issue = running.issue(found)
    if not match or not issue:
        return "?", "no brief named"
    thread = comments(issue)
    if thread is None:
        return None
    briefs = rebriefs.briefs_by_issue(thread).get(issue, [])
    for position, comment in enumerate(briefs):
        if str(comment.get("id")) == match.group(1):
            return ("first", "") if position == 0 else ("round", rebriefs.round_of(comment.get("body", "")))
    return "?", "brief not found"


def measure(found: dict, times: runlog.Times, briefed: tuple[str, str]) -> dict:
    """One session's record: seconds, from its fire; gates inside `init`…end, and the
    reading's share of them up to the first edit, or all of them with no edit."""
    fired = running.fired_at(found)
    run = found.get("last_run") if isinstance(found.get("last_run"), dict) else {}

    def since(at):
        return round((at - fired).total_seconds()) if at and fired else None

    start, end = times.init, times.end
    return {"issue": running.issue(found), "kind": briefed[0], "cause": briefed[1],
            "fired": fired.strftime("%Y-%m-%dT%H:%M:%SZ") if fired else None,
            "init": since(start), "first_edit": since(times.first_edit), "end": since(end),
            "turns": times.turns, "listed": run_end(run),
            "gates_reading": round(times.gate_seconds(start, times.first_edit or end)),
            "gates": round(times.gate_seconds(start, end))}


def read(listed: list[dict], kept: dict, ask, comments: Callable[[int], list | None]) -> Read:
    """Measure every fired coder routine in `listed` not already in `kept`."""
    done = Read()
    for found in listed:
        number = running.issue(found)
        run = found.get("last_run") if isinstance(found.get("last_run"), dict) else {}
        session = str(run.get("session_id") or "")
        if number is None or not run:
            continue
        if not SESSION.match(session):
            done.skipped.append(f"#{number}: no plain session id listed")
            continue
        if session in kept:
            if kept[session].get("listed") == run_end(run):
                continue
            done.stale.append(session)
        if running.fired_at(found) is None:
            done.skipped.append(f"#{number} {session}: no fire time listed")
            continue
        try:
            log = pages(session, ask)
        except remote.RemoteError as down:
            done.skipped.append(f"#{number} {session}: {down}")
            continue
        times = runlog.read(runlog.lines(log or []))
        if log is None or times.running or times.init is None:
            why = "log longer than the pages read" if log is None else "still running" if times.running else "no init line"
            done.skipped.append(f"#{number} {session}: {why}")
            continue
        briefed = kind(found, comments)
        if briefed is None:
            done.skipped.append(f"#{number} {session}: the issue's comments unread")
            continue
        done.records[session] = measure(found, times, briefed)
    return done


def load(path) -> dict:
    got = files.read(path)
    return {k: v for k, v in got.items() if isinstance(v, dict)} if isinstance(got, dict) else {}


def split(record: dict) -> dict[str, float] | None:
    """A record's seconds as sandbox, reading, gates and the rest, which sum to its `end`;
    `None` when it lacks a time. With no first edit, every gate is inside the reading."""
    init, edit, end = number(record, "init"), number(record, "first_edit"), number(record, "end")
    if init is None or end is None or not 0 <= init <= end or end <= 0:
        return None
    edit = end if edit is None else min(max(edit, init), end)
    gates = max(0, number(record, "gates") or 0)
    before = gates if number(record, "first_edit") is None else number(record, "gates_reading") or 0
    before = min(max(0, before), gates, edit - init)
    after = min(gates - before, end - edit)
    return {"sandbox": init, "reading": edit - init - before, "gates": before + after, "rest": end - edit - after}


def row(session: str, record: dict) -> str:
    told = str(record.get("kind") or "?") + (f" ({record['cause']})" if record.get("cause") else "")
    parts = split(record)
    if parts is None:
        return f"  #{record.get('issue')} {told} {session}: times unreadable"
    return (f"  #{record.get('issue')} {told} fired {record.get('fired')} {session}: "
            f"total {sum(parts.values())}s, sandbox {parts['sandbox']}s, reading {parts['reading']}s, "
            f"gates {parts['gates']}s, rest {parts['rest']}s, turns {record.get('turns')}")


def summary(label: str, records: list[dict]) -> list[str]:
    parts = [p for p in map(split, records) if p]
    if not parts:
        return [f"{label}: none measured"]
    cold = sorted(100 * (p["sandbox"] + p["reading"]) / sum(p.values()) for p in parts)
    whole = {k: sum(p[k] for p in parts) for k in parts[0]}
    total = sum(whole.values()) or 1
    share = ", ".join(f"{k} {100 * v / total:.0f}%" for k, v in whole.items())
    return [f"{label}: {len(parts)} sessions; cold start (sandbox + reading) median {statistics.median(cold):.0f}%"
            f" of a session, {cold[0]:.0f}–{cold[-1]:.0f}%",
            f"  where the time goes: {share}"]


def report(records: dict[str, dict], fresh: int, skipped: list[str]) -> str:
    lines = [f"coder-cost: {len(records)} sessions measured ({fresh} read now, the rest kept in .git/{FILE});"
             f" the routines API lists only the account's {LISTED} newest routines",
             "coder-cost: tokens are not in the run log: not counted"]
    lines += [f"  skipped {why}" for why in skipped]
    ordered = sorted(records.items(), key=lambda item: str(item[1].get("fired") or ""))
    lines += [row(session, record) for session, record in ordered]
    lines += summary("first briefs", [r for r in records.values() if r.get("kind") == "first"])
    lines += summary("rounds", [r for r in records.values() if r.get("kind") == "round"])
    return "\n".join(lines)
