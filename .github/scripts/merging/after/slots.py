"""The waiting and running mutation requests, and the cap between them ([`pump`], #1934)."""

from __future__ import annotations

import json
import os
import secrets
from pathlib import Path

# Where the requests live, under the checkout's git directory beside the
# hand-back memory, so a relaunched watch picks up what the last one left.
FILE = "merge-queue/mutants.json"

# At most this many scoped runs at once. Each holds one runner for up to
# 2 h 45; two leaves the rest of the account's concurrent jobs to the gating
# `ci` and `lint` runs, which on 2026-11-08 waited 20-40 minutes behind six.
CAP = 2

# A dispatched run that has not shown up in the run list after this long is
# taken as lost and its slot freed: a dispatch returns no run id to ask after.
LOST_SECONDS = 15 * 60

# A request GitHub refuses this many times in a row is dropped, and said.
TRIES = 3


def empty() -> dict:
    return {"waiting": [], "running": []}


def load(path: Path) -> dict:
    """The requests on disk; none when the file is missing or unreadable."""
    try:
        state = json.loads(path.read_text())
    except (OSError, ValueError):
        return empty()
    if not isinstance(state, dict):
        return empty()
    return {key: [e for e in state.get(key) or [] if readable(e)] for key in ("waiting", "running")}


def readable(entry: object) -> bool:
    """Whether a stored request has what dispatching it needs; one that does not is dropped."""
    return (isinstance(entry, dict) and isinstance(entry.get("pr"), int)
            and isinstance(entry.get("scope"), str) and isinstance(entry.get("commit"), str))


def save(path: Path, state: dict) -> str | None:
    """Write the requests whole (a temporary file, then a rename); why not, if it failed."""
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        part = path.with_suffix(".part")
        part.write_text(json.dumps(state, indent=1, sort_keys=True))
        os.replace(part, path)
    except OSError as failed:
        return str(failed)
    return None


def enqueue(state: dict, number: int, scope: str, commit: str) -> bool:
    """Add a merged pull request's request; `False` when it is already known."""
    if any(e.get("pr") == number for e in state["waiting"] + state["running"]):
        return False
    state["waiting"].append({"pr": number, "scope": scope, "commit": commit})
    return True


def found(runs: list[dict], request: str) -> dict | None:
    """The run a request started, recognised by the id the workflow puts in its name."""
    tag = f"[{request}]"
    return next((r for r in runs if tag in (r.get("display_title") or "")), None)


def free(state: dict, runs: list[dict] | None, now: float, say) -> bool:
    """Drop the running requests whose run finished or never appeared. `runs`
    `None` (GitHub did not answer) frees nothing: a slot is never guessed free."""
    if runs is None:
        return False
    kept = []
    for entry in state["running"]:
        run = found(runs, entry.get("request", ""))
        if run is not None and run.get("status") == "completed":
            say(f"after: #{entry['pr']}'s mutation run ended {run.get('conclusion')}: {run.get('html_url', '')}")
        elif run is None and now - float(entry.get("since", now)) > LOST_SECONDS:
            say(f"after: #{entry['pr']}'s mutation run never appeared; its slot is freed.")
        else:
            kept.append(entry)
    changed = len(kept) != len(state["running"])
    state["running"] = kept
    return changed


def pump(state: dict, runs: list[dict] | None, dispatch, now: float, say, token=None) -> bool:
    """Free what finished, then dispatch the oldest waiting requests while a
    slot is open. `dispatch(entry, request)` says why GitHub refused, or `None`.
    A refusal keeps the request first in line for the next pass, until [`TRIES`].
    Returns whether the state changed."""
    token = token or (lambda: secrets.token_hex(3))
    changed = free(state, runs, now, say)
    while state["waiting"] and len(state["running"]) < CAP:
        entry = state["waiting"][0]
        request = f"pr{entry['pr']}-{token()}"
        refused = dispatch(entry, request)
        changed = True
        if refused is None:
            state["waiting"].pop(0)
            state["running"].append({**entry, "request": request, "since": now})
            say(f"after: dispatched #{entry['pr']}'s mutation run ({entry['scope']}) as {request}.")
            continue
        entry["tries"] = int(entry.get("tries", 0)) + 1
        if entry["tries"] >= TRIES:
            state["waiting"].pop(0)
            say(f"after: GitHub refused #{entry['pr']}'s mutation run {TRIES} times; dropped: {refused}")
            continue
        say(f"after: GitHub refused #{entry['pr']}'s mutation run; asking again next pass: {refused}")
        break
    return changed
