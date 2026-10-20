"""A red run that is GitHub's runners' fault, not the code's (#869).

The answer to one is a re-run, not a hand-back ([`queue.polling.rerun_step`]).

## Whose red it is waits for the gate (#1109)

The runs listing and each run's jobs listing are separate reads, and either can
lag the other ([`runlist.lagging`], #1095). A run listed `completed/failure`
whose jobs do not yet show the gate concluded on the run's own attempt (the
gate unconcluded, not listed, or listed only for an older attempt) has not said
whose red it is: judging it then read an empty `bad` and handed an outage back
as a code failure. So [`runner_red`] answers *not yet known* there, and the
queue waits ([`unread`]), bounded by `--deadline` as any wait is.
"""

from __future__ import annotations

from merging.verdict import runlist


def runner_red(run: dict, jobs: dict[int, list[dict]], workflow: str) -> bool | None:
    """Whether this failed run is red for GitHub's runners, not the code (#869);
    `None` while a `failure`'s jobs do not show its gate concluded (#1109).

    On 2026-10-05 hosted runners stopped being acquired and every run in flight
    came back red with nothing compiled, in two shapes: the gate itself
    `cancelled` beside green jobs, and `changes` `cancelled` so the gate
    `failure`d over skipped jobs. So: some job was cancelled, and nothing but
    the gate failed. A run cancelled with no job to show for it counts too.
    A job still going (a signal, [`runlist.by_gate`]) is not evidence either way.
    """
    if run.get("conclusion") == "failure" and runlist.gate_conclusion(run, jobs, workflow) is None:
        return None
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


def judged(pull: dict, runs: list[dict], jobs: dict[int, list[dict]]) -> list[tuple[str, dict, bool | None]]:
    """Every failed run on the head, as (workflow, run, [`runner_red`])."""
    sha = pull.get("headRefOid", "")
    return [
        (workflow, run, runner_red(run, jobs, workflow))
        for workflow in runlist.WORKFLOWS
        for run in runlist.failed_runs(runlist.live(runlist.by_gates(runlist.runs_for(runs, sha, workflow), jobs, workflow)))
    ]


def outage(pull: dict, runs: list[dict], jobs: dict[int, list[dict]]) -> list[dict]:
    """The failed runs on the head when every one of them is [`runner_red`];
    empty when any failed for the code, any is not yet known, or none failed.
    A re-run, not a hand-back, is the answer to these (`merge-queue.py`)."""
    reds = judged(pull, runs, jobs)
    return [run for _, run, red in reds] if all(red is True for _, _, red in reds) else []


def unread(pull: dict, runs: list[dict], jobs: dict[int, list[dict]]) -> list[tuple[str, dict]]:
    """The failed runs whose [`runner_red`] is not yet known, as (workflow, run);
    empty when any failed run is already the code's, which is answer enough."""
    reds = judged(pull, runs, jobs)
    if any(red is False for _, _, red in reds):
        return []
    return [(workflow, run) for workflow, run, red in reds if red is None]
