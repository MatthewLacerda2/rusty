#!/usr/bin/env python3
"""`coder.py N [--dry-run] [--no-wait] [--over-limit]` — start the cloud coder for issue #N (#1569).

`make coder ISSUE=N` is this. It reads the newest comment on #N whose first line
is `Brief`, and creates a one-off routine that starts a cloud session on it a
minute or two later, with a prompt
that points the coder at `cloud-brief.md` and at the URL of the exact comment
checked here, so a comment posted after the launch cannot stand in for it
(`dispatch/routine.py` has the body and why each field is what it is). Running
it again for the same issue is the re-brief launch: the new routine names the
newest `Brief` at that moment.

It refuses, and starts nothing, when the issue is closed, is labelled
`planning` or `human`, has no brief, or has a brief from someone who cannot push
(`dispatch/brief.py`). When [`running.LIMIT`] coders on other issues are
already waiting to fire or running, it puts the issue in the waiting line
(`dispatch/waiting.py`, #2382) and says how many are ahead; `make coders`
launches it when a slot frees, and `--no-wait` refuses instead. A free slot is
not enough while the line holds others: an issue that is not within the free
slots in the line's order waits too, and says who is ahead, so a hand launch
never jumps a round that has waited an hour. The decision and the create hold
the line's lock and re-list the routines first, as a `make coders` launch does
(`dispatch/drain.py`) (#1856; a fired one is placed from its pull
request and briefs on GitHub, #1866, `dispatch/running.py` says why; a ready pull
request with no review record yet still holds its slot, named `in review`, #2017, and a draft whose issue has an open
recorded blocker frees it, named `draft blocked by #M`, #2063): a re-brief of an issue is never counted against
itself, and `--over-limit` launches anyway.

The operator asked on 2026-10-29 for a script that starts the coder for an
issue, so spawning one is no work: before it, the orchestrator assembled the
`RemoteTrigger` body by hand from `issue-batch` each time, and the operator
could not launch one without a Claude session.

`--dry-run` prints the routine it would create and creates nothing; at the
limit it says where the issue would wait, writes nothing and takes no lock. Needs the
`claude` CLI signed in to claude.ai (the routines API is reached through
`claude mcp serve`, `dispatch/remote.py`) and `gh`. `RUSTY_CLOUD_ENV` names the
`anthropic_cloud` environment's id when no coder routine exists yet to read it
from. Exits 0 on a launch or a place in the line, 1 on a refusal or a usage
error, 2 when GitHub or claude.ai failed, the line could not be written (after
a launch too: the next `make coders` then drops the entry by its routine) or
another session held the line past `waiting.WAIT_SECONDS`.
"""

from __future__ import annotations

import contextlib
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import ghrest
from dispatch import brief, drain, launch, remote, running, waiting

USAGE = "usage: coder.py ISSUE [--dry-run] [--no-wait] [--over-limit]"
FLAGS = ("--dry-run", "--no-wait", "--over-limit")


def say(message: str) -> None:
    print(f"coder: {message}", file=sys.stderr, flush=True)


def gh_json(path: str) -> object:
    done = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        return None
    return json.loads(done.stdout)


def counted(listed: list[dict], number: int, reader: running.GitHub) -> list[tuple[dict, str]]:
    """The coders that count against #`number`'s launch; says the uncounted ones worth naming."""
    found, noted = running.coders(listed, number, reader)
    for routine, placed, why in noted:
        name = routine.get("name", "?")
        say(f"cannot tell whether {name} is still running ({why}); not counted" if placed == "unknown"
            else f"{name}: {why}; not counted")
    return found


def full(found: list[tuple[dict, str]]) -> str:
    names = "; ".join(running.describe(routine, why) for routine, why in found)
    return f"{len(found)} cloud coders already waiting or running, the limit is {running.LIMIT} (CLAUDE.md): {names}"


def wait(line: Path, entries: list[dict], number: int, found: dict, now: datetime, dry: bool, why: str) -> int:
    """Put #`number` in the waiting line `entries` (or, `dry`, say where it would go)."""
    entries, added = waiting.enqueue(entries, number, found, now)
    if not dry and (failed := waiting.save(line, entries)):
        say(f"could not write the waiting line {line}: {failed}")
        return 2
    verb = "would wait" if dry else ("waiting" if added else "already waiting")
    say(f"{why}. #{number} {verb} for a slot, {waiting.ahead(entries, number)} ahead: `make coders` "
        "launches it when one frees (ARGS=--no-wait refuses instead, --over-limit launches now)")
    return 0


def turn(entries: list[dict], number: int, found: dict, now: datetime, free: int) -> str | None:
    """Why #`number` must wait behind the line for one of `free` slots, or `None` when it fits:
    a hand launch never jumps ahead of an entry asked before it in the line's order."""
    placed, _ = waiting.enqueue([dict(e) for e in entries], number, found, now)
    ahead = waiting.ordered(placed)[:waiting.ahead(placed, number)]
    if len(ahead) < free:
        return None
    names = ", ".join(f"#{e['issue']}" + (" (round)" if e.get("round") else "") for e in ahead)
    return f"{free} free slot(s) and {len(ahead)} ahead in the waiting line: {names}"


def decide(argv: list[str], number: int, issue: dict, found: dict, listed: list[dict],
           holding: list[tuple[dict, str]], call, now: datetime, env: dict, line: Path) -> int:
    """Launch or queue #`number` on what is true now: the line and the routines read again."""
    dry = "--dry-run" in argv
    entries = waiting.load(line)
    taken = {running.issue(found) for found, _ in holding}
    if not dry:
        taken |= drain.fresh(launch.routines(call), listed) - {number}
    free = running.LIMIT - len(taken)
    behind = full(holding) if free <= 0 else turn(entries, number, found, now, free)
    if behind and "--over-limit" in argv:
        say(f"--over-limit: launching past {behind}")
    elif behind and "--no-wait" in argv:
        raise brief.Refused(behind + ". Launch when one is ready and reviewed, or pass --over-limit")
    elif behind:
        return wait(line, entries, number, found, now, dry, behind)
    status = launch.create(number, issue, found, listed, call, ghrest.slug(), now, env, dry, say)
    if status == 0 and not dry and any(e["issue"] == number for e in entries):
        if failed := waiting.save(line, waiting.without(entries, number)):
            say(f"launched, but could not take #{number} out of the waiting line {line}: {failed}; "
                "the next `make coders` drops it by its routine")
            return 2
    return status


def main(argv: list[str], gh=gh_json, call=remote.call, now: datetime | None = None,
         env: dict | None = None, line: Path | None = None) -> int:
    args = [a for a in argv if a not in FLAGS]
    if len(args) != 1 or not args[0].lstrip("#").isdigit():
        say(USAGE)
        return 1
    number, dry = int(args[0].lstrip("#")), "--dry-run" in argv
    now = now or datetime.now(timezone.utc)
    line = line or waiting.path()
    repository = ghrest.slug()
    issue = gh(f"repos/{repository}/issues/{number}")
    comments = ghrest.pages(gh, f"repos/{repository}/issues/{number}/comments")
    if not isinstance(issue, dict) or comments is None:
        say(f"could not read #{number} from GitHub")
        return 2
    try:
        found = brief.read(issue, comments)
        listed = launch.routines(call)
        holding = counted(listed, number, launch.github(gh, repository, now))
        # the decision and the create under the line's lock, as `make coders` launches (dispatch/drain.py)
        with contextlib.nullcontext() if dry else waiting.held(line, say):
            return decide(argv, number, issue, found, listed, holding, call, now,
                          os.environ if env is None else env, line)
    except ValueError as refused:  # brief.Refused, or a brief URL that is not this issue's
        say(f"refused: {refused}")
        return 1
    except remote.RemoteError as failed:  # waiting.Busy too
        say(str(failed))
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
