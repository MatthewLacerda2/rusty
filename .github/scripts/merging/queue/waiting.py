"""The polling loop around [`polling.progress`], until the runs on a head settle."""

from __future__ import annotations

import subprocess
import time

from merging.queue import outcomes, polling, pulls
from merging.verdict import github, runners


# The deadline's last line when its last poll listed a run red whose jobs had
# not yet shown it over (#1831's review): "Nothing is red" would be wrong, and
# the watch says how it ended a listing later ([`watch.stalls`]).
STALLED_RED = (
    "Handed back with a run listed red whose jobs do not show it over yet, so whether the runners"
    " or the code reddened it is not known; the watch says how it ends."
)


def wait_for(
    repo: str, number: int, sha: str, before: str, deadline: float, poll: float, tick=None
) -> tuple[str, list[str]]:
    """Poll until the runs on `sha` settle, or the deadline says stop.

    Never treats an absent check as a settled one — [`polling.progress`] tells them
    apart, and [`polling.head_state`] does the same one level up. What has been
    [`polling.sighted`] on `sha` is remembered across polls, so a run the listing
    drops is waited for rather than declared lost (#592). A red only the
    runners caused is re-run once ([`polling.rerun_step`]) on the same clock,
    and a red whose jobs do not yet show it over is given a grace from the
    poll that first read it so ([`polling.lagging`], #1109, #1182), then
    judged as its jobs are listed ([`polling.expiry`]). Past `deadline` with
    the runs still out it ends [`outcomes.STALLED`], not a stop (#1831).
    `tick()`, when given, runs once per poll: the watch reports its board
    there, so a pull request turning ready during a long run is heard of
    within a poll, not when the take ends (#928).
    """
    began, seen, rerun, red_since = time.monotonic(), set(), {}, {}
    while True:
        pull = pulls.look(number)
        waited = time.monotonic() - began
        state, lines = polling.head_state(pull.get("headRefOid", ""), sha, before, waited)
        red = False  # a run listed red whose jobs don't show it over yet, this poll
        if state in (outcomes.STOP, outcomes.HEAD_MOVED):
            return state, lines
        if state == outcomes.GO:
            runs, jobs = github.evidence(repo, sha)
            seen |= polling.sighted(pull, runs)
            state, lines = polling.progress(pull, runs, jobs, github.paths(pull), waited, seen, rerun, red_since)
            red = state == outcomes.WAIT and bool(runners.unread(pull, runs, jobs))
            if state == outcomes.RERUN:
                refused = rerun_failed(runners.outage(pull, runs, jobs, polling.expiry(waited, red_since)), rerun)
                if refused:
                    return outcomes.STOP, [f"{outcomes.RUNNERS}, and the re-run was refused: {refused}", *lines]
            elif state != outcomes.WAIT:
                return state, lines
        if waited > deadline:
            # Its own state, not a red one: the watch brings it back by itself
            # when the run it outwaited goes green ([`watch.stalls`], #1831).
            return outcomes.STALLED, [
                f"still waiting on {sha[:7]} after {waited / 60:.0f} minutes.",
                *lines,
                STALLED_RED if red else "Handed back with the run still out. Nothing is red; nothing is green either.",
            ]
        outcomes.say(f"#{number}: {lines[0]}")
        if tick is not None:
            tick()
        time.sleep(poll)


def rerun_failed(stalled: list[dict], rerun: dict[int, int]) -> str | None:
    """`gh run rerun --failed` on each run, recording the attempt it had;
    GitHub's refusal, or `None` once all of them were asked."""
    for run in stalled:
        done = subprocess.run(
            ["gh", "run", "rerun", "--failed", str(run["id"])], capture_output=True, text=True, check=False
        )
        if done.returncode != 0:
            return done.stderr.strip() or f"gh run rerun exited {done.returncode}"
        rerun[run["id"]] = run.get("run_attempt", 1)
    return None
