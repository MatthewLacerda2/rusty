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

# A request GitHub refuses this many times in a row is dropped, and said. A
# network failure is not a refusal and never counts (#847's lesson, the
# independent review of PR #1935: a one-minute outage used to burn all three).
TRIES = 3

KEYS = ("unread", "waiting", "running")


class Busy(str):
    """Why a call failed for the network's reasons, not GitHub's answer: the
    request keeps its place and its tries ([`pump`])."""


def empty() -> dict:
    return {key: [] for key in KEYS}


def load(path: Path, say=lambda line: None) -> dict:
    """The requests on disk; none when the file is missing or unreadable. An
    entry that cannot be read is dropped and said, never raised (a watch killed
    by its own state file stops every merge, not one signal)."""
    try:
        state = json.loads(path.read_text())
    except FileNotFoundError:
        return empty()
    except (OSError, ValueError) as failed:
        say(f"after: the mutation requests in {path} are unreadable; dropped: {failed}")
        return empty()
    if not isinstance(state, dict):
        say(f"after: the mutation requests in {path} are unreadable; dropped.")
        return empty()
    kept = empty()
    for key in KEYS:
        entries = state.get(key, [])
        entries = entries if isinstance(entries, list) else [entries]
        kept[key] = [e for e in entries if readable(key, e)]
        if len(kept[key]) != len(entries):
            say(f"after: {len(entries) - len(kept[key])} unreadable {key} mutation request(s) dropped.")
    return kept


def number(value: object) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def readable(key: str, entry: object) -> bool:
    """Whether a stored request has, in the right types, what handling it needs."""
    if not isinstance(entry, dict) or not isinstance(entry.get("pr"), int) or isinstance(entry["pr"], bool):
        return False
    if not number(entry.get("tries", 0)):
        return False
    if key == "unread":
        return True
    if not (isinstance(entry.get("scope"), str) and isinstance(entry.get("commit"), str)):
        return False
    return key == "waiting" or (isinstance(entry.get("request"), str) and number(entry.get("since")))


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


def known(state: dict, pr: int) -> bool:
    return any(e["pr"] == pr for key in KEYS for e in state[key])


def enqueue(state: dict, pr: int, scope: str, commit: str) -> bool:
    """Add a merged pull request's request; `False` when it is already known."""
    if any(e["pr"] == pr for e in state["waiting"] + state["running"]):
        return False
    state["waiting"].append({"pr": pr, "scope": scope, "commit": commit})
    return True


def found(runs: list[dict], request: str) -> dict | None:
    """The run a request started, recognised by the id the workflow puts in its name."""
    tag = f"[{request}]"
    return next((r for r in runs if isinstance(r, dict) and tag in (r.get("display_title") or "")), None)


def free(state: dict, runs: list[dict] | None, now: float, say) -> bool:
    """Drop the running requests whose run finished or never appeared. `runs`
    `None` (GitHub did not answer) frees nothing: a slot is never guessed free."""
    if runs is None:
        return False
    kept = []
    for entry in state["running"]:
        run = found(runs, entry["request"])
        if run is not None and run.get("status") == "completed":
            say(f"after: #{entry['pr']}'s mutation run ended {run.get('conclusion')}: {run.get('html_url', '')}")
        elif run is None and now - float(entry["since"]) > LOST_SECONDS:
            say(f"after: #{entry['pr']}'s mutation run never appeared; its slot is freed.")
        else:
            kept.append(entry)
    changed = len(kept) != len(state["running"])
    state["running"] = kept
    return changed


def failed(entry: dict, why: str, what: str, say) -> bool:
    """Count a failed call against `entry`; whether it is now dropped. A
    [`Busy`] failure counts nothing and is said once, on its first pass."""
    if isinstance(why, Busy):
        if not entry.get("busy"):
            say(f"after: GitHub did not answer for #{entry['pr']}'s {what}; asking again next pass: {why}")
        entry["busy"] = True
        return False
    entry["busy"] = False
    entry["tries"] = int(entry.get("tries", 0)) + 1
    if entry["tries"] >= TRIES:
        say(f"after: GitHub refused #{entry['pr']}'s {what} {TRIES} times; dropped: {why}")
        return True
    say(f"after: GitHub refused #{entry['pr']}'s {what}; asking again next pass: {why}")
    return False


def pump(state: dict, runs: list[dict] | None, dispatch, now: float, say, token=None) -> bool:
    """Free what finished, then dispatch the oldest waiting requests while a
    slot is open. `dispatch(entry, request)` says why it failed ([`Busy`] for
    the network), or `None`. A failure keeps the request first in line for the
    next pass; a refusal counts toward [`TRIES`]. Returns whether the state changed."""
    token = token or (lambda: secrets.token_hex(3))
    changed = free(state, runs, now, say)
    while state["waiting"] and len(state["running"]) < CAP:
        entry = state["waiting"][0]
        request = f"pr{entry['pr']}-{token()}"
        why = dispatch(entry, request)
        changed = True
        if why is None:
            state["waiting"].pop(0)
            entry.pop("tries", None), entry.pop("busy", None)
            state["running"].append({**entry, "request": request, "since": now})
            say(f"after: dispatched #{entry['pr']}'s mutation run ({entry['scope']}) as {request}.")
            continue
        if failed(entry, why, "mutation run", say):
            state["waiting"].pop(0)
            continue
        break
    return changed
