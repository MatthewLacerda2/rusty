"""The polling loop around [`polling.progress`], until the runs on a head settle."""

from __future__ import annotations

import subprocess
import time

from merging.queue import outcomes, polling, pulls
from merging.verdict import github, runners


def wait_for(
    repo: str, number: int, sha: str, before: str, deadline: float, poll: float, tick=None
) -> tuple[str, list[str]]:
    """Poll until the runs on `sha` settle, or the deadline says stop.

    Never treats an absent check as a settled one — [`polling.progress`] tells them
    apart, and [`polling.head_state`] does the same one level up. What has been
    [`polling.sighted`] on `sha` is remembered across polls, so a run the listing
    drops is waited for rather than declared lost (#592). A red only the
    runners caused is re-run once ([`polling.rerun_step`]) on the same clock.
    `tick()`, when given, runs once per poll: the watch reports its board
    there, so a pull request turning ready during a long run is heard of
    within a poll, not when the take ends (#928).
    """
    began, seen, rerun = time.monotonic(), set(), {}
    while True:
        pull = pulls.look(number)
        waited = time.monotonic() - began
        state, lines = polling.head_state(pull.get("headRefOid", ""), sha, before, waited)
        if state == outcomes.STOP:
            return state, lines
        if state == outcomes.GO:
            runs, jobs = github.evidence(repo, sha)
            seen |= polling.sighted(pull, runs)
            state, lines = polling.progress(pull, runs, jobs, github.paths(pull), waited, seen, rerun)
            if state == outcomes.RERUN:
                refused = rerun_failed(runners.outage(pull, runs, jobs), rerun)
                if refused:
                    return outcomes.STOP, [f"{outcomes.RUNNERS}, and the re-run was refused: {refused}", *lines]
            elif state != outcomes.WAIT:
                return state, lines
        if waited > deadline:
            return outcomes.STOP, [
                f"still waiting on {sha[:7]} after {waited / 60:.0f} minutes.",
                *lines,
                "Handed back with the run still out. Nothing is red; nothing is green either.",
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
