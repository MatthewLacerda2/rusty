"""A red run that is GitHub's runners' fault, not the code's (#869).

The answer to one is a re-run, not a hand-back ([`queue.polling.rerun_step`]).
"""

from __future__ import annotations

from merging.verdict import runlist


def runner_red(run: dict, jobs: dict[int, list[dict]], workflow: str) -> bool:
    """Whether this failed run is red for GitHub's runners, not the code (#869).

    On 2026-10-05 hosted runners stopped being acquired and every run in flight
    came back red with nothing compiled, in two shapes: the gate itself
    `cancelled` beside green jobs, and `changes` `cancelled` so the gate
    `failure`d over skipped jobs. So: some job was cancelled, and nothing but
    the gate failed. A run cancelled with no job to show for it counts too.
    A job still going (a signal, [`runlist.by_gate`]) is not evidence either way.
    """
    gate = runlist.WORKFLOWS[workflow][0]
    bad = [
        j for j in jobs.get(run.get("id"), [])
        if j.get("conclusion") not in (None, "success", "skipped")
    ]
    if not bad:
        return run.get("conclusion") == "cancelled"
    return any(j.get("conclusion") == "cancelled" for j in bad) and all(
        j.get("conclusion") == "cancelled" or (j.get("name") == gate and j.get("conclusion") == "failure")
        for j in bad
    )


def outage(pull: dict, runs: list[dict], jobs: dict[int, list[dict]]) -> list[dict]:
    """The failed runs on the head when every one of them is [`runner_red`];
    empty when any failed for the code, or none failed. A re-run, not a
    hand-back, is the answer to these (`merge-queue.py`)."""
    sha, stalled = pull.get("headRefOid", ""), []
    for workflow in runlist.WORKFLOWS:
        for run in runlist.failed_runs(runlist.live(runlist.by_gates(runlist.runs_for(runs, sha, workflow), jobs, workflow))):
            if not runner_red(run, jobs, workflow):
                return []
            stalled.append(run)
    return stalled
