#!/usr/bin/env python3
"""`coder.py N [--dry-run]` — start the cloud coder for issue #N (#1569).

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
(`dispatch/brief.py`), and when [`running.LIMIT`] coders on other issues are
already waiting to fire or running (#1856; a fired one is placed from its pull
request and briefs on GitHub, #1866, `dispatch/running.py` says why): a re-brief of an issue is never counted against
itself, and `--over-limit` launches anyway.

The operator asked on 2026-10-29 for a script that starts the coder for an
issue, so spawning one is no work: before it, the orchestrator assembled the
`RemoteTrigger` body by hand from `issue-batch` each time, and the operator
could not launch one without a Claude session.

`--dry-run` prints the routine it would create and creates nothing; it still
refuses at the limit, since that is what a launch would do. Needs the
`claude` CLI signed in to claude.ai (the routines API is reached through
`claude mcp serve`, `dispatch/remote.py`) and `gh`. `RUSTY_CLOUD_ENV` names the
`anthropic_cloud` environment's id when no coder routine exists yet to read it
from. Exits 0 on a launch, 1 on a refusal, 2 when GitHub or claude.ai failed.
"""

from __future__ import annotations

import functools
import json
import os
import subprocess
import sys
from datetime import datetime, timezone

import ghrest
from dispatch import brief, remote, routine, running

USAGE = "usage: coder.py ISSUE [--dry-run] [--over-limit]"
FLAGS = ("--dry-run", "--over-limit")


def say(message: str) -> None:
    print(f"coder: {message}", file=sys.stderr, flush=True)


def gh_json(path: str) -> object:
    done = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        return None
    return json.loads(done.stdout)


def routines(call) -> list[dict]:
    """The account's routines, as the routines API lists them."""
    status, answer = call({"action": "list"})
    if status != 200 or not isinstance(answer, dict):
        raise remote.RemoteError(f"listing routines answered HTTP {status}: {str(answer)[:300]}")
    return answer.get("data") or []


def environment_id(env: dict, listed: list[dict]) -> str:
    """`RUSTY_CLOUD_ENV`, or the environment the newest coder routine ran in."""
    if env.get("RUSTY_CLOUD_ENV"):
        return env["RUSTY_CLOUD_ENV"]
    found = routine.environment(listed)
    if found is None:
        raise brief.Refused("no coder routine exists to take the environment from: set RUSTY_CLOUD_ENV "
                            "to the id of the anthropic_cloud environment (claude.ai/code, Environments)")
    return found


def github(gh, repository: str, now: datetime) -> running.GitHub:
    """What placing a fired coder reads from GitHub, over REST (#1866): one listing of open
    pull requests, and per issue its comments and, when it has no open pull request, itself
    and its closed pull requests. Those come from one page of the newest closed pull requests,
    read at most once (#1913): a coder's PR closed after its fire is hours old at most."""
    pulls = ghrest.pages(gh, f"repos/{repository}/pulls?state=open")
    if pulls is None:
        raise remote.RemoteError("could not list the open pull requests from GitHub")
    newest_closed = functools.cache(lambda: gh(f"repos/{repository}/pulls?state=closed&sort=updated"
                                               f"&direction=desc&per_page={ghrest.PER_PAGE}"))

    def closed(number: int) -> list | None:
        got = newest_closed()
        return running.pulls_for(number, got) if isinstance(got, list) else None

    return running.GitHub(pulls=pulls,
                          comments=lambda n: ghrest.pages(gh, f"repos/{repository}/issues/{n}/comments"),
                          issue=lambda n: gh(f"repos/{repository}/issues/{n}"), now=now, closed=closed)


def check_limit(listed: list[dict], number: int, over: bool, reader: running.GitHub) -> None:
    """Refuse a launch past [`running.LIMIT`] coders, unless `over`; name what was counted."""
    counted, noted = running.coders(listed, number, reader)
    for found, placed, why in noted:
        name = found.get("name", "?")
        say(f"cannot tell whether {name} is still running ({why}); not counted" if placed == "unknown"
            else f"{name}: {why}; not counted")
    if len(counted) < running.LIMIT:
        return
    names = "; ".join(running.describe(found) for found in counted)
    if over:
        say(f"--over-limit: launching past {len(counted)} running coders ({names})")
        return
    raise brief.Refused(f"{len(counted)} cloud coders already waiting or running, the limit is {running.LIMIT} "
                        f"(CLAUDE.md): {names}. Launch when one is ready, or pass --over-limit")


def trigger_id(answer: object) -> str | None:
    if not isinstance(answer, dict):
        return None
    return (answer.get("trigger") or answer).get("id")


def main(argv: list[str], gh=gh_json, call=remote.call, now: datetime | None = None,
         env: dict | None = None) -> int:
    args = [a for a in argv if a not in FLAGS]
    if len(args) != 1 or not args[0].lstrip("#").isdigit():
        say(USAGE)
        return 1
    number, dry = int(args[0].lstrip("#")), "--dry-run" in argv
    now = now or datetime.now(timezone.utc)
    repository = ghrest.slug()
    issue = gh(f"repos/{repository}/issues/{number}")
    comments = ghrest.pages(gh, f"repos/{repository}/issues/{number}/comments")
    if not isinstance(issue, dict) or comments is None:
        say(f"could not read #{number} from GitHub")
        return 2
    try:
        found = brief.read(issue, comments)
        listed = routines(call)
        check_limit(listed, number, "--over-limit" in argv, github(gh, repository, now))
        env_id = environment_id(os.environ if env is None else env, listed)
        made = routine.body(number, issue.get("title", ""), found.get("html_url", ""), env_id, repository,
                            now)
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
    print(f"coder: #{number} on {routine.MODEL}, starts at {made['run_once_at']} (brief {found.get('html_url', '')})")
    print(f"coder: routine {made_id}: {routine.ROUTINES_URL}{made_id} (its run links the session)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
