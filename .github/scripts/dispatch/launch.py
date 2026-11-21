"""Creating a coder's routine, shared by `make coder` and the line `make coders` launches.

Both launch the same way (#2382): the routine [`create`] makes is the one
`dispatch/routine.py` builds on the brief `brief.read` checked, in the cloud
environment [`environment_id`] finds. What reads GitHub to place a fired coder
([`github`]) is here too, so both count the running coders from one reader.
"""

from __future__ import annotations

import functools
import json
from datetime import datetime
from typing import Callable

import ghrest
from dispatch import brief, remote, routine, running


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
    pull requests, and per issue its comments and, when its pull request is a draft, its
    recorded blockers (#2063), or, when it has no open pull request, itself and its closed
    pull requests. Those come from one page of the newest closed pull requests,
    read at most once (#1913): a coder's PR closed after its fire is hours old at most."""
    pulls = ghrest.pages(gh, f"repos/{repository}/pulls?state=open")
    if pulls is None:
        raise remote.RemoteError("could not list the open pull requests from GitHub")
    newest_closed = functools.cache(lambda: gh(f"repos/{repository}/pulls?state=closed&sort=updated"
                                               f"&direction=desc&per_page={ghrest.PER_PAGE}"))

    def closed(number: int) -> list | None:
        got = newest_closed()
        return running.pulls_for(number, got) if isinstance(got, list) else None

    def blockers(number: int) -> list[int] | None:
        got = gh(ghrest.blocked_by(repository, number))
        return ghrest.open_blockers(got) if isinstance(got, list) else None

    return running.GitHub(pulls=pulls, blockers=blockers,
                          comments=lambda n: ghrest.pages(gh, f"repos/{repository}/issues/{n}/comments"),
                          issue=lambda n: gh(f"repos/{repository}/issues/{n}"), now=now, closed=closed)


def trigger_id(answer: object) -> str | None:
    if not isinstance(answer, dict):
        return None
    return (answer.get("trigger") or answer).get("id")


def create(number: int, issue: dict, found: dict, listed: list[dict], call, repository: str,
           clock: Callable[[], datetime], env: dict, dry: bool, say: Callable[[str], None]) -> int:
    """Create (or, `dry`, print) the routine for #`number` on the brief `found`: 0 when made or
    printed, 2 when claude.ai failed. Raises `brief.Refused` when no environment is known.

    The fire time is taken from `clock` here, as the create is made, never from a time the
    command read at its start (the review of #2393): `routine.fire_time` leaves as little as a
    minute, and a wait on the line's lock, a re-listing or the creates before this one in a pass
    can spend it, and the API refuses a fire time already past."""
    env_id = environment_id(env, listed)
    model = routine.model_for(issue.get("body"))
    made = routine.body(number, issue.get("title", ""), found.get("html_url", ""), env_id, repository, clock(),
                        model=model)
    if dry:
        print(json.dumps(made, indent=2))
        say(f"dry run: nothing created; it would run on {model} (brief {found.get('html_url', '')})")
        return 0
    status, answer = call({"action": "create", "body": made})
    made_id = trigger_id(answer)
    if status not in (200, 201) or not made_id:
        say(f"creating the routine answered HTTP {status}: {str(answer)[:500]}")
        return 2
    print(f"coder: #{number} on {model}, starts at {made['run_once_at']} (brief {found.get('html_url', '')})")
    print(f"coder: routine {made_id}: {routine.ROUTINES_URL}{made_id} (its run links the session)")
    return 0
