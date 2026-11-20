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
  to the last `result:` (the work), the turns, and the gates' wall-clock, split
  at the first edit.
- **First brief or round** is read from GitHub: the routine's prompt names its
  brief by URL (`routine.PROMPT`); its place among the issue's trusted briefs
  (`rebriefs.briefs_by_issue`) says which, and a round's `Round:` line its cause.
- **The listing holds only the account's 20 newest routines** (`routine.py`,
  `ENVIRONMENT_FROM`), so what each run measured is kept in [`FILE`] under the
  common git directory, by session id, through `merging/state.py`; a later run
  adds to it, and a session already kept is not read again. A session still
  running is not kept: it is named and read again next time.
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
    """One pass: the records it measured, and the sessions it skipped with why."""

    records: dict[str, dict] = field(default_factory=dict)
    skipped: list[str] = field(default_factory=list)


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


def kind(found: dict, comments: Callable[[int], list | None]) -> tuple[str, str]:
    """`first` or `round` and the round's cause (`""` for a first brief), or `?` and why."""
    match = BRIEF.search(prompt(found))
    number = running.issue(found)
    thread = comments(number) if match and number else None
    if thread is None:
        return "?", "brief unread"
    briefs = rebriefs.briefs_by_issue(thread).get(number, [])
    for position, comment in enumerate(briefs):
        if str(comment.get("id")) == match.group(1):
            return ("first", "") if position == 0 else ("round", rebriefs.round_of(comment.get("body", "")))
    return "?", "brief not found"


def measure(found: dict, times: runlog.Times, briefed: tuple[str, str]) -> dict:
    """One session's record: seconds, from its fire."""
    fired = running.fired_at(found)

    def since(at):
        return round((at - fired).total_seconds()) if at and fired else None

    edit = times.first_edit
    return {"issue": running.issue(found), "kind": briefed[0], "cause": briefed[1],
            "fired": fired.strftime("%Y-%m-%dT%H:%M:%SZ") if fired else None,
            "init": since(times.init), "first_edit": since(edit), "end": since(times.end),
            "turns": times.turns, "gates_reading": round(times.gate_seconds(stop=edit)) if edit else 0,
            "gates": round(times.gate_seconds())}


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
            continue
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
        done.records[session] = measure(found, times, kind(found, comments))
    return done


def load(path) -> dict:
    got = files.read(path)
    return {k: v for k, v in got.items() if isinstance(v, dict)} if isinstance(got, dict) else {}


def split(record: dict) -> dict[str, float] | None:
    """A record's seconds as sandbox, reading, gates and the rest; `None` when it lacks a time."""
    init, edit, end = record.get("init"), record.get("first_edit"), record.get("end")
    if init is None or end is None or end <= 0:
        return None
    edit = end if edit is None else edit
    gates = record.get("gates") or 0
    reading = max(0, edit - init - (record.get("gates_reading") or 0))
    return {"sandbox": init, "reading": reading, "gates": gates, "rest": max(0, end - init - reading - gates)}


def row(session: str, record: dict) -> str:
    told = record["kind"] + (f" ({record['cause']})" if record.get("cause") else "")
    parts = split(record)
    if parts is None:
        return f"  #{record.get('issue')} {told} {session}: times unreadable"
    return (f"  #{record.get('issue')} {told} fired {record.get('fired')} {session}: "
            f"total {record['end']}s, sandbox {parts['sandbox']}s, reading {parts['reading']}s, "
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
    ordered = sorted(records.items(), key=lambda item: item[1].get("fired") or "")
    lines += [row(session, record) for session, record in ordered]
    lines += summary("first briefs", [r for r in records.values() if r.get("kind") == "first"])
    lines += summary("rounds", [r for r in records.values() if r.get("kind") == "round"])
    return "\n".join(lines)
