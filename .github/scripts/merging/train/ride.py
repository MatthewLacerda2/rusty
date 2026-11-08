"""Waiting for the gates on a train's tip ([`wait`]), on the queue's own polling rules.

The tip is no pull request's head, so it is judged as one with nothing to claim:
[`queue.polling.progress`] over a pull request of only `headRefOid`, whose
description rules (`verdict.protocol`) have nothing to read and refuse nothing.
Every reading rule a pull request's wait has holds here unchanged: a run not
listed yet waits out its grace, a runner outage is re-run once (#869), a red whose
jobs lag waits for them (#1109, #1182), a run seen once and dropped from the
listing is still waited for (#592). The members' own claims were judged before
the train formed, as [`queue.taking.take`] judges them before its push.
"""

from __future__ import annotations

import time

from merging.queue import outcomes, polling, waiting
from merging.verdict import decide, github, runlist, runners


# How a train's wait ends besides [`outcomes.GO`]: its gates said no, or nothing
# said anything in time (a runner outage or a deadline: not the trees' fault).
# Either way the train is split and its halves are tested next
# ([`watch.trains.Yard.ride`]); only a red one marks its members as red, so a
# stalled member may ride a new train again once its half is done.
RED, STALLED = "red", "stalled"


def failed(tip: dict, runs: list[dict], jobs: dict[int, list[dict]], expired: runners.Expired = runners.patient) -> bool:
    """Whether a gating workflow's run on the tip failed for the code's reasons.
    `expired` is the wait's own ([`polling.expiry`]): without it a run cancelled
    again past the jobs' grace read as the code's, and a runner outage sent every
    member to a CI round of its own (#1785's review)."""
    sha = tip["headRefOid"]
    return any(
        decide.assess(tip, w, runlist.runs_for(runs, sha, w), jobs)[0] == decide.FAILED
        and not runners.outage(tip, runs, jobs, expired)
        for w in runlist.WORKFLOWS
    )


def wait(
    repo: str, sha: str, deadline: float, poll: float, tick=None, say=outcomes.say, files: tuple[str, ...] | list[str] = ()
) -> tuple[str, list[str]]:
    """Poll until the gates on `sha` settle: [`outcomes.GO`], [`RED`] or
    [`STALLED`], with why. Past `deadline` seconds it is stalled, nothing red.
    `files` are every member's changed paths: the tip carries them as a pull
    request does, so a member editing what the setup steps run makes a setup
    failure the train's code's ([`verdict.setup.edited`], #1932).
    A poll GitHub fails (out of retries, or a `gh` refusal) is *wait*, until the
    deadline (#1785's review: one HTTP 500 at minute 20 stopped the watch, and its
    relaunch built a new sha, so every member paid the CI round again)."""
    began, seen, rerun, red_since = time.monotonic(), set(), {}, {}
    while True:
        waited = time.monotonic() - began
        try:
            runs, jobs = github.evidence(repo, sha)
            tip = {"headRefOid": sha, "isDraft": False, "statusCheckRollup": github.rollup(repo, sha),
                   "files": [{"path": p} for p in files]}
        except (github.Unreachable, SystemExit) as down:
            lines = [f"GitHub did not answer this poll ({down}); waiting on."]
        else:
            seen |= polling.sighted(tip, runs)
            state, lines = polling.progress(tip, runs, jobs, [], waited, seen, rerun, red_since)
            if state == outcomes.RERUN:
                refused = waiting.rerun_failed(runners.outage(tip, runs, jobs, polling.expiry(waited, red_since)), rerun)
                if refused:
                    return STALLED, [f"{outcomes.RUNNERS}, and the re-run was refused: {refused}", *lines]
            elif state == outcomes.GO:
                return state, lines
            elif state != outcomes.WAIT:
                # Red only for a run that failed: a run cancelled again after its one
                # re-run (#869), or none that ever built the tip, is not the tree's.
                red = failed(tip, runs, jobs, polling.expiry(waited, red_since))
                return (RED if red else STALLED), lines
        if waited > deadline:
            return STALLED, [f"still waiting on the train {sha[:7]} after {waited / 60:.0f} minutes.", *lines]
        say(f"train {sha[:7]}: {lines[0]}")
        if tick is not None:
            tick()
        time.sleep(poll)
