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

from merging.queue import outcomes


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
)


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
        ["gh", "pr", "view", str(number), "--json", "state,mergedAt"],
        capture_output=True, text=True, check=False,
    )
    if done.returncode != 0:
        return None
    try:
        return json.loads(done.stdout)
    except json.JSONDecodeError:
        return None


def confirm(number: int, poll: float) -> tuple[str, list[str]]:
    """Ask, until [`settled`] says something other than *wait*."""
    began = time.monotonic()
    while True:
        state, lines = settled(merge_record(number), time.monotonic() - began)
        if state != outcomes.WAIT:
            return state, lines
        outcomes.say(f"#{number}: {lines[0]}")
        time.sleep(min(poll, MERGE_SETTLES_SECONDS / 4))
