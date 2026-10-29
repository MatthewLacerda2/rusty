#!/usr/bin/env python3
"""`coder.py N [--dry-run]` — start the cloud coder for issue #N (#1569).

`make coder ISSUE=N` is this. It reads the newest comment on #N whose first line
is `Brief`, takes the model from its `Model:` line, and creates a one-off
routine that starts a cloud session on it a minute or two later, with a prompt
that points the coder at `cloud-brief.md` and at the URL of the exact comment
checked here, so a comment posted after the launch cannot stand in for it
(`dispatch/routine.py` has the body and why each field is what it is). Running
it again for the same issue is the re-brief launch: the new routine names the
newest `Brief` at that moment.

It refuses, and starts nothing, when the issue is closed, is labelled
`planning` or `human`, has no brief, has a brief from someone who cannot push,
or a brief with no `Model:` line naming Opus or Sonnet (`dispatch/brief.py`).

The operator asked on 2026-10-29 for a script that starts the coder for an
issue, so spawning one is no work: before it, the orchestrator assembled the
`RemoteTrigger` body by hand from `issue-batch` each time, and the operator
could not launch one without a Claude session.

`--dry-run` prints the routine it would create and creates nothing. Needs the
`claude` CLI signed in to claude.ai (the routines API is reached through
`claude mcp serve`, `dispatch/remote.py`) and `gh`. `RUSTY_CLOUD_ENV` names the
`anthropic_cloud` environment's id when no coder routine exists yet to read it
from. Exits 0 on a launch, 1 on a refusal, 2 when GitHub or claude.ai failed.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from datetime import datetime, timezone

import ghrest
from dispatch import brief, remote, routine

USAGE = "usage: coder.py ISSUE [--dry-run]"


def say(message: str) -> None:
    print(f"coder: {message}", file=sys.stderr, flush=True)


def gh_json(path: str) -> object:
    done = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        return None
    return json.loads(done.stdout)


def environment_id(env: dict, call) -> str:
    """`RUSTY_CLOUD_ENV`, or the environment the newest coder routine ran in."""
    if env.get("RUSTY_CLOUD_ENV"):
        return env["RUSTY_CLOUD_ENV"]
    status, answer = call({"action": "list"})
    if status != 200 or not isinstance(answer, dict):
        raise remote.RemoteError(f"listing routines answered HTTP {status}: {str(answer)[:300]}")
    found = routine.environment(answer.get("data") or [])
    if found is None:
        raise brief.Refused("no coder routine exists to take the environment from: set RUSTY_CLOUD_ENV "
                            "to the id of the anthropic_cloud environment (claude.ai/code, Environments)")
    return found


def trigger_id(answer: object) -> str | None:
    if not isinstance(answer, dict):
        return None
    return (answer.get("trigger") or answer).get("id")


def main(argv: list[str], gh=gh_json, call=remote.call, now: datetime | None = None,
         env: dict | None = None) -> int:
    args = [a for a in argv if a != "--dry-run"]
    if len(args) != 1 or not args[0].lstrip("#").isdigit():
        say(USAGE)
        return 1
    number, dry = int(args[0].lstrip("#")), "--dry-run" in argv
    repository = ghrest.slug()
    issue = gh(f"repos/{repository}/issues/{number}")
    comments = ghrest.pages(gh, f"repos/{repository}/issues/{number}/comments")
    if not isinstance(issue, dict) or comments is None:
        say(f"could not read #{number} from GitHub")
        return 2
    try:
        found, model = brief.read(issue, comments)
        env_id = environment_id(os.environ if env is None else env, call)
        made = routine.body(number, issue.get("title", ""), found.get("html_url", ""), model, env_id, repository,
                            now or datetime.now(timezone.utc))
        if dry:
            print(json.dumps(made, indent=2))
            say(f"dry run: nothing created (brief {found.get('html_url', '')})")
            return 0
        status, answer = call({"action": "create", "body": made})
    except ValueError as refused:  # brief.Refused, or a brief URL that is not this issue's
        say(f"refused: {refused}")
        return 1
    except remote.RemoteError as failed:
        say(str(failed))
        return 2
    made_id = trigger_id(answer)
    if status not in (200, 201) or not made_id:
        say(f"creating the routine answered HTTP {status}: {str(answer)[:500]}")
        return 2
    print(f"coder: #{number} on {model}, starts at {made['run_once_at']} (brief {found.get('html_url', '')})")
    print(f"coder: routine {made_id}: {routine.ROUTINES_URL}{made_id} (its run links the session)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
