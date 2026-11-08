"""A red run that is GitHub's runners' fault, not the code's (#869).

The answer to one is a re-run, not a hand-back ([`queue.polling.rerun_step`]).

## Whose red it is waits for the gate (#1109)

The runs listing and each run's jobs listing are separate reads, and either can
lag the other ([`runlist.lagging`], #1095). A run listed `completed/failure`
whose jobs do not yet show the gate concluded on the run's own attempt (the
gate unconcluded, not listed, or listed only for an older attempt) has not said
whose red it is: judging it then read an empty `bad` and handed an outage back
as a code failure. A job other than the gate still listed going is the same
lag (#1182): [`runner_red`] reads the jobs only once they [`runlist.agrees`]
the run is over, whatever the run concluded. So [`runner_red`] answers *not yet
known* there, and the queue waits ([`unread`]) for a grace from the poll that
first read it so ([`queue.polling.lagging`]).

Past the grace the jobs are judged as listed, the reading before #1182: only a
`failure` whose gate has not concluded stays unknown (#1109), and the queue
hands it back, red. A runner outage whose listing still shows a job going is
therefore still re-run (#869), not handed back as the code's.

## A job that failed installing a tool is the runners' too (#1932)

A job that failed in a setup step ([`setup.failed_in_setup`]: fetching a tool,
the toolchain, system libraries, the cargo-deny image) concludes `failure`,
not `cancelled`, with nothing built. It counts as a cancelled job does: re-run
once inside the same deadline, and a second such failure hands back as
`runners unavailable …`. A train red only this way is re-run, not split
([`train.ride.failed`] reads the same [`outage`]). A pull request whose own
files change what the setup steps run ([`setup.edited`]) is judged on its
setup failures: they are the code's. [`judged`] reads that off the pull
request's `files`, which a train's tip carries for all its members.
"""

from __future__ import annotations

from typing import Callable

from merging.verdict import github, runlist, setup

# Whether the grace for `run`'s jobs listing to catch up is spent
# ([`queue.polling.expiry`]); without one, it never is.
Expired = Callable[[dict], bool]


def patient(run: dict) -> bool:
    """The default [`Expired`]: the grace is never spent."""
    return False


def runner_red(
    run: dict, jobs: dict[int, list[dict]], workflow: str, expired: Expired = patient, own_setup: bool = False
) -> bool | None:
    """Whether this failed run is red for GitHub's runners, not the code (#869);
    `None` while its jobs do not [`runlist.agrees`] it is over (#1182), until
    `expired`, and past it while a `failure`'s gate has not concluded (#1109).

    On 2026-10-05 hosted runners stopped being acquired and every run in flight
    came back red with nothing compiled, in two shapes: the gate itself
    `cancelled` beside green jobs, and `changes` `cancelled` so the gate
    `failure`d over skipped jobs. So: some job was cancelled, and nothing but
    the gate failed. A run cancelled with no job to show for it counts too.
    A job still going (a signal, [`runlist.by_gate`]) is not evidence either way.
    A job that failed in a setup step alone ([`setup.failed_in_setup`], #1932)
    counts as a cancelled one, unless `own_setup` (the pull request edits what
    its setup runs, [`setup.edited`]).
    """
    if not runlist.agrees(run, jobs, workflow) and not expired(run):
        return None
    if run.get("conclusion") == "failure" and runlist.gate_conclusion(run, jobs, workflow) is None:
        return None
    gate = runlist.WORKFLOWS[workflow][0]
    bad = _bad(run, jobs)
    if not bad:
        return run.get("conclusion") == "cancelled"
    runners_own = [j for j in bad if j.get("conclusion") == "cancelled" or (not own_setup and setup.failed_in_setup(j))]
    return bool(runners_own) and all(
        j in runners_own or (j.get("name") == gate and j.get("conclusion") == "failure") for j in bad
    )


def _bad(run: dict, jobs: dict[int, list[dict]]) -> list[dict]:
    return [j for j in jobs.get(run.get("id"), []) if j.get("conclusion") not in (None, "success", "skipped")]


def why(run: dict, jobs: dict[int, list[dict]]) -> str:
    """What made a [`runner_red`] run red, for a message: each job but the gate,
    cancelled or the setup step it failed in (#1932's review: name the step)."""
    gate = runlist.WORKFLOWS.get(run.get("name", ""), ("",))[0]
    said = [
        f"`{j.get('name')}` cancelled" if j.get("conclusion") == "cancelled"
        else f"`{j.get('name')}` failed in `{setup.step(j)}`"
        for j in _bad(run, jobs) if j.get("name") != gate
    ]
    return ", ".join(said) or "cancelled"


def judged(
    pull: dict, runs: list[dict], jobs: dict[int, list[dict]], expired: Expired = patient
) -> list[tuple[str, dict, bool | None]]:
    """Every failed run on the head, as (workflow, run, [`runner_red`])."""
    sha, own = pull.get("headRefOid", ""), setup.edited(github.paths(pull))
    return [
        (workflow, run, runner_red(run, jobs, workflow, expired, own))
        for workflow in runlist.WORKFLOWS
        for run in runlist.failed_runs(runlist.live(runlist.by_gates(runlist.runs_for(runs, sha, workflow), jobs, workflow)))
    ]


def outage(
    pull: dict, runs: list[dict], jobs: dict[int, list[dict]], expired: Expired = patient
) -> list[dict]:
    """The failed runs on the head when every one of them is [`runner_red`];
    empty when any failed for the code, any is not yet known, or none failed.
    A re-run, not a hand-back, is the answer to these (`merge-queue.py`)."""
    reds = judged(pull, runs, jobs, expired)
    return [run for _, run, red in reds] if all(red is True for _, _, red in reds) else []


def unread(
    pull: dict, runs: list[dict], jobs: dict[int, list[dict]], expired: Expired = patient
) -> list[tuple[str, dict]]:
    """The failed runs whose [`runner_red`] is not yet known, as (workflow, run);
    empty when any failed run is already the code's, which is answer enough."""
    reds = judged(pull, runs, jobs, expired)
    if any(red is False for _, _, red in reds):
        return []
    return [(workflow, run) for workflow, run, red in reds if red is None]
