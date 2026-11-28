#!/usr/bin/env python3
"""`orchestrator.py [--dry-run | --stop | status | take | renew | release | queue MINUTES | outbox [--sent ID]]` (#2667).

`make orchestrator` is this: the cloud orchestrator, an hourly routine whose
fresh session runs one fire of the loop (`.claude/agents/orchestrator.md`).

- **bare: create the schedule** (`orchestrating/schedule.py`), from this
  machine, through `claude mcp serve` as `make coder` reaches the routines API.
  Refused when one is enabled already. `--dry-run` prints the body and creates
  nothing; `--stop` disables every enabled one (a routine is never deleted by a
  script, so its run history stays).
- **`status`**: the lock, one line (`orchestrating/lease.py`); exit 0 when
  free or this side's, 4 when another holds it.
- **`take`**, **`renew`**, **`release`**: the fire's hold on the lock, as
  `cloud <session id>` in a cloud session (`CLAUDE_CODE_REMOTE_SESSION_ID`),
  else `local@<host> hand`. `take` restores the carried queue state
  (`orchestrating/carry.py`) and exits 4, naming the holder, when another
  holds it: the fire then ends. After a take, a `last <time>` line gives
  the lease it replaced (its release, or its holder's last renewal): the
  `--since` of the fire's drain pass; none when the lock was never taken.
  `renew` and `release` save the state.
- **`queue MINUTES [watch flags]`**: one watch (`merge-queue.py --watch
  --no-check --for MINUTES`) under the lock, renewed in the background; asked
  to stop between pull requests if the lock is lost.
- **`outbox`**: the `create_session` arguments of every launch the scripts
  queued in `RUSTY_ROUTINES`' file (`dispatch/handed.py`), one JSON object a
  line with its `id`; `--sent ID` marks one sent once the session made it.

Any other flag is a usage error and creates nothing; `-h`/`--help` prints this.
Exits 0 done, 1 refused or a usage error, 2 when GitHub or claude.ai failed,
4 when another orchestrator holds the lock; `queue` exits the watch's status.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import ghrest
from dispatch import brief, handed, launch, remote
from dispatch.routine import ROUTINES_URL
from merging.watch import ending
from orchestrating import keeper, lease, schedule, side

USAGE = "usage: orchestrator.py [--dry-run | --stop | status | take | renew | release | queue MINUTES | outbox [--sent ID]]"


def say(message: str) -> None:
    print(f"orchestrator: {message}", file=sys.stderr, flush=True)


def me(env: dict) -> str:
    if env.get("CLAUDE_CODE_REMOTE_SESSION_ID"):
        return lease.holder("cloud", env["CLAUDE_CODE_REMOTE_SESSION_ID"])
    return lease.holder(lease.local_place(), "hand")


def common(root: str) -> Path:
    done = subprocess.run(["git", "rev-parse", "--git-common-dir"], cwd=root, capture_output=True, text=True,
                          check=False)
    return Path(root, done.stdout.strip() or ".git")


def scheduled(argv: list[str], call, env: dict, now: datetime) -> int:
    if argv not in ([], ["--stop"], ["--dry-run"]):
        say(USAGE)
        return 1
    listed = launch.routines(call)
    running = schedule.ours(listed)
    if "--stop" in argv:
        for found in running:
            status, answer = call({"action": "update", "trigger_id": found.get("id"), "body": {"enabled": False}})
            if status not in (200, 201):
                say(f"disabling {found.get('id')} answered HTTP {status}: {str(answer)[:300]}")
                return 2
            print(f"orchestrator: disabled {found.get('id')}")
        if not running:
            say("no enabled orchestrator routine")
        return 0
    if running:
        say(f"refused: {schedule.NAME} is enabled already ({running[0].get('id')}); `ARGS=--stop` disables it")
        return 1
    made = schedule.body(launch.environment_id(env, listed), ghrest.slug(), now)
    if "--dry-run" in argv:
        print(json.dumps(made, indent=2))
        say("dry run: nothing created")
        return 0
    status, answer = call({"action": "create", "body": made})
    made_id = launch.trigger_id(answer)
    if status not in (200, 201) or not made_id:
        say(f"creating the routine answered HTTP {status}: {str(answer)[:500]}")
        return 2
    print(f"orchestrator: {schedule.NAME} fires on `{schedule.CRON}`: {ROUTINES_URL}{made_id}")
    return 0


def locked(verb: str, mine: side.Side) -> int:
    if verb == "status":
        found = lease.read(mine.git)
        print(f"orchestrator: lock {side.describe(found)}")
        return lease.HELD_STATUS if lease.refusal(found, mine.me, mine.clock()) else 0
    if verb == "take":
        refused, restored = mine.take()
        if refused:
            say(refused)
            return lease.HELD_STATUS
        print(f"orchestrator: lock held by {mine.me}" + (f"; restored {', '.join(restored)}" if restored else ""))
        if mine.before is not None and mine.before.at is not None:
            print(f"orchestrator: last {lease.stamp(mine.before.at)} ({side.describe(mine.before)})")
        return 0
    why = mine.renew() if verb == "renew" else mine.release()
    if why:
        say(why)
        return lease.HELD_STATUS
    print(f"orchestrator: lock {'renewed' if verb == 'renew' else 'released'} by {mine.me}")
    return 0


def queue(minutes: str, flags: list[str], mine: side.Side, root: str) -> int:
    if not minutes.replace(".", "", 1).isdigit():
        say(USAGE)
        return 1
    if lease.read(mine.git).holder != mine.me:
        say(f"refused: take the lock first (`{sys.argv[0]} take`); the queue runs only under it")
        return 1

    def lost(why: str) -> None:
        say(f"{why}; asking the watch to stop between pull requests")
        (mine.directory.parent / ending.STOP_FILE).touch()
    command = [sys.executable, str(Path(root, ".github", "scripts", "merge-queue.py")), "--watch", "--no-check",
               "--for", minutes, *flags]
    with keeper.Keeper(mine.renew, lost, say):
        status = subprocess.run(command, cwd=root, check=False).returncode
    mine.renew()
    return status


def outbox(argv: list[str]) -> int:
    file = handed.path()
    if file is None:
        say(f"{handed.ENV} names no file: nothing was queued for `create_session`")
        return 1
    data = handed.load(file)
    if "--sent" in argv:
        wanted = argv[argv.index("--sent") + 1] if argv.index("--sent") + 1 < len(argv) else ""
        entry = next((e for e in data.get("outbox") or [] if e.get("id") == wanted), None)
        if entry is None:
            say(f"no outbox entry {wanted!r}")
            return 1
        entry["sent"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        handed.save(file, data)
        return 0
    for entry in data.get("outbox") or []:
        if not entry.get("sent"):
            print(json.dumps({"id": entry["id"], "create_session": handed.session_args(entry["body"])}))
    return 0


def main(argv: list[str], call=remote.call, env: dict | None = None, git=None, root: str = ".") -> int:
    env = os.environ if env is None else env
    if argv[:1] in (["-h"], ["--help"]):
        print(__doc__)
        return 0
    verb = argv[0] if argv and not argv[0].startswith("-") else None
    try:
        if verb is None:
            return scheduled(argv, call, env, datetime.now(timezone.utc))
        if verb == "outbox":
            return outbox(argv[1:])
        mine = side.Side(git or side.git_in(root), me(env), common(root) / "merge-queue")
        if verb in ("status", "take", "renew", "release") and len(argv) == 1:
            return locked(verb, mine)
        if verb == "queue" and len(argv) >= 2:
            return queue(argv[1], argv[2:], mine, root)
    except (lease.Unreachable, OSError, remote.RemoteError) as failed:
        say(str(failed))
        return 2
    except brief.Refused as refused:
        say(f"refused: {refused}")
        return 1
    say(USAGE)
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
