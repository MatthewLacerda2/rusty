"""The merge call, and what to believe when it fails in transit.

## GitHub's answer about a state is not always the state

The runs listing is eventually consistent ([`polling.progress`]'s grace), a force-push
lags its own poll ([`polling.head_state`]'s), and a merge call can answer **502 having
already merged** (scorsese#495). So a failed merge is sorted by what failed: a
transport failure is followed by asking whether it merged ([`transport`],
[`landed`]); a refusal GitHub reasoned about is handed back with no second
call.
"""

from __future__ import annotations

import json
import re
import subprocess
import time

import ghrest
from merging.queue import gitio, outcomes, pulls


# How long a merge call that failed in transit is given to show up as merged.
MERGE_SETTLES_SECONDS = 60

# What a failure *in transit* looks like in `gh`'s stderr, as opposed to a
# refusal: a REST 5xx, a GraphQL `HTTP 5xx`, or Go's network errors verbatim.
TRANSPORT_STATUS = re.compile(r"(status code:|HTTP)\s*5\d\d\b", re.IGNORECASE)

TRANSPORT_WORDS = (
    "timeout",
    "timed out",
    "deadline exceeded",
    "connection reset",
    "connection refused",
    "broken pipe",
    "unexpected eof",
    # `gh` cut off mid-answer: #1713's merge call printed it on 2026-11-04 and a
    # healthy pull request was handed back for nothing.
    "unexpected end of json input",
)

# GitHub's 405 to a merge call made right after another landed on the base
# (#1785's review): the merge train squashes its members back to back. It is
# asked again after each wait here; still refused, it is [`BASE_MOVED`].
BASE_MODIFIED = "base branch was modified"
BASE_RETRIES = (3, 10, 30)


def transport(stderr: str) -> bool:
    """Whether a failed merge call failed *in transit* rather than being refused.

    Errs towards *refusal*: an unrecognised message is handed back without a
    second look, which never reports a merge as done that was not.
    """
    lowered = stderr.lower()
    return bool(TRANSPORT_STATUS.search(stderr)) or any(w in lowered for w in TRANSPORT_WORDS)


def landed(pull: dict) -> bool:
    """Whether GitHub's record says it merged. Not ancestry: a squash is a new
    commit, so the branch head is never an ancestor of `main`."""
    return pull.get("state") == "MERGED" or bool(pull.get("mergedAt"))


def settled(pull: dict | None, waited: float) -> tuple[str, list[str]]:
    """After a merge call failed in transit: merged, not yet known, or unknown.

    `None` is GitHub not answering either — no more an answer than the 5xx.
    Past the window it is handed back as *unknown*, never as refused: "refused"
    about a merge that happened invites a second one.
    """
    if pull is not None and landed(pull):
        return outcomes.GO, ["the merge call failed in transit, but it merged."]
    if waited < MERGE_SETTLES_SECONDS:
        return outcomes.WAIT, ["the merge call failed in transit; asking whether it merged."]
    return outcomes.STOP, [
        "the merge call failed in transit and GitHub has not said it merged.",
        "Whether it did is unknown: look at the pull request before merging it again.",
    ]


def merge_record(number: int) -> dict | None:
    """The merge fields, or `None` if GitHub did not answer — asked during the
    very outage that made it necessary, so never through the fatal `gh`."""
    done = subprocess.run(
        ["gh", "api", f"repos/{ghrest.slug()}/pulls/{number}"],
        capture_output=True, text=True, check=False,
    )
    if done.returncode != 0:
        return None
    try:
        pull = ghrest.pull(json.loads(done.stdout))
    except json.JSONDecodeError:
        return None
    return {"state": pull["state"], "mergedAt": pull["mergedAt"]}


def confirm(number: int, poll: float) -> tuple[str, list[str]]:
    """Ask, until [`settled`] says something other than *wait*."""
    began = time.monotonic()
    while True:
        state, lines = settled(merge_record(number), time.monotonic() - began)
        if state != outcomes.WAIT:
            return state, lines
        outcomes.say(f"#{number}: {lines[0]}")
        time.sleep(min(poll, MERGE_SETTLES_SECONDS / 4))


def squash_message(fresh: str, root: str) -> str:
    """The squash commit's body: a bullet per commit the branch made, oldest first,
    its merges of `main` left out, so `main` carries no `Merge remote-tracking
    branch` bullets (#1568). Empty when git cannot say."""
    done = gitio.git("log", "--no-merges", "--reverse", "--format=%s", f"origin/main..{fresh}", cwd=root)
    if done.returncode != 0:
        return ""
    return "\n".join(f"* {line.strip()}" for line in done.stdout.splitlines() if line.strip())


def squash(repo: str, pull: dict, sha: str, root: str, poll: float, runner=None, sleep=None) -> tuple[str, str]:
    """Squash-merge `pull` at head `sha`: [`outcomes.MERGED`], or why not.

    REST's merge (#970); `sha` is `--match-head-commit`, so a push after the
    verdict makes GitHub refuse. A refusal is [`outcomes.HANDED_BACK`], except
    "Base branch was modified", asked again on [`BASE_RETRIES`] and then
    [`BASE_MOVED`]; a call that failed in transit is asked about ([`confirm`]),
    and one GitHub never says merged is [`UNKNOWN`]: the merge train must not
    land past it.
    """
    number, runner, sleep = pull["number"], runner or subprocess.run, sleep or time.sleep
    call = ["gh", "api", "--method", "PUT", f"repos/{repo}/pulls/{number}/merge",
            "-f", "merge_method=squash", "-f", f"sha={sha}", "-f", f"commit_title={pulls.subject(pull)}",
            "-f", f"commit_message={squash_message(sha, root)}"]
    for wait in (*BASE_RETRIES, None):
        done = runner(call, capture_output=True, text=True, check=False)
        if done.returncode == 0:
            return outcomes.MERGED, "merged."
        failure = done.stderr.strip()
        if BASE_MODIFIED not in failure.lower():
            break
        if wait is None:
            outcomes.say(f"#{number}: GitHub still says its base branch was modified: {failure}")
            return BASE_MOVED, f"GitHub still says its base branch was modified after {len(BASE_RETRIES)} retries: {failure}"
        outcomes.say(f"#{number}: GitHub says its base branch was modified; asking again in {wait}s.")
        sleep(wait)
    if not transport(failure):
        blocked = f"CI passed but the merge was refused: {failure}"
        outcomes.say(f"#{number}: {blocked}")
        return outcomes.HANDED_BACK, blocked
    outcomes.say(f"#{number}: the merge call failed in transit: {failure}")
    state, after = confirm(number, poll)
    outcomes.say(f"#{number}: {after[0]}", *after[1:])
    return (outcomes.MERGED, after[0]) if state == outcomes.GO else (UNKNOWN, after[0])


# What [`squash`] returns when the call failed in transit and GitHub never said it merged.
UNKNOWN = "merge unknown"

# What [`squash`] returns when GitHub kept saying the base moved: no fault of the
# pull request's, so it goes back in line, never handed back.
BASE_MOVED = "base moved"
