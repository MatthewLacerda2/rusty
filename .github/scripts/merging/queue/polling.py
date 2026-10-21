"""One poll's answer: wait, go, re-run, or hand back ([`progress`]).

## A runner outage is re-run, not handed back

When GitHub's hosted runners stop being acquired, every run in flight comes
back red at once with nothing compiled (2026-10-05, #869): the only jobs not
green are `cancelled`, or a gate that failed because the job it gates was. That
is GitHub's failure, not the branch's, so [`waiting.wait_for`] re-runs those runs once
(`gh run rerun --failed`, [`rerun_step`]) and keeps waiting inside the same
`--deadline`; only a second cancellation hands back, as [`outcomes.RUNNERS`]. A red with
any job `failure` besides a gate is the code's, and hands back at once as ever.
A red whose jobs do not yet show the gate concluded is neither: it waits
([`verdict.runners.unread`], #1109), but only for [`GATE_SHOWS_SECONDS`] from
when this wait first read it so. A run that failed before any job started (an
invalid workflow file, say) never lists a gate; waited on to the deadline it
was handed back as "Nothing is red", when it is red and, before #1109, was
handed back on the first poll. Past the grace it is judged as listed: FAILED,
with its URL.
"""

from __future__ import annotations

from merging.queue import outcomes
from merging.verdict import absent, decide, runlist, runners


# How long "nothing has built this yet" is read as *not yet*. A push and the
# runs it creates are not simultaneous, and the listing is eventually
# consistent; after this, the silence is the answer ([`verdict.absent.no_run`]).
RUN_APPEARS_SECONDS = 300

# How long a run listed failed whose jobs do not show its gate concluded is read
# as the jobs listing lagging (#1109), from the poll that first read it so: as
# long as a run is given to appear, since both are the same eventual consistency.
GATE_SHOWS_SECONDS = RUN_APPEARS_SECONDS


def head_state(seen: str, pushed: str, before: str, waited: float) -> tuple[str, list[str]]:
    """Whether the head GitHub reports is the one this queue is watching.

    The pushed head goes; the pre-push head waits while the grace lasts
    (GitHub's view lags the push); a third sha is somebody else and stops —
    merging a commit this queue never watched is not given the benefit of the
    doubt.
    """
    if seen == pushed:
        return outcomes.GO, []
    if seen == before and waited < RUN_APPEARS_SECONDS:
        return outcomes.WAIT, [f"GitHub still reports {before[:7]}; it has not caught up with the push yet."]
    return outcomes.STOP, [
        f"the head is {seen[:7]}, not the {pushed[:7]} this queue is watching.",
        "Somebody else pushed. Handed back rather than merging a commit this"
        " queue never watched.",
    ]


def sighted(pull: dict, runs: list[dict]) -> set[str]:
    """The workflows with a run on the head that GitHub has shown, by the runs
    listing or by the pull request's own checks ([`verdict.absent.in_rollup`])."""
    sha = pull.get("headRefOid", "")
    return {
        w for w in runlist.WORKFLOWS if runlist.runs_for(runs, sha, w) or absent.in_rollup(pull, w)
    }


def rerun_step(stalled: list[dict], rerun: dict[int, int], short: str) -> tuple[str, list[str]]:
    """What to do about runs red only for the runners (#869): re-run them,
    wait for the re-run to show, or hand back as [`outcomes.RUNNERS`].

    `rerun` maps each run this wait re-ran to the attempt it had then. A run
    not in it is re-run ([`outcomes.RERUN`]); one whose listed attempt is not past
    that is the listing lagging the re-run, and waits; one already re-run
    and red again is the hand-back. Once per run, inside the same deadline.
    """
    names = ", ".join(sorted({f"`{r.get('name')}`" for r in stalled}))
    if any(r.get("id") not in rerun for r in stalled):
        return outcomes.RERUN, [
            f"{names} on {short} came back cancelled with nothing failed: GitHub's runners, not the branch.",
            "Re-running the cancelled jobs once (#869).",
        ]
    if any(r.get("run_attempt", 1) <= rerun[r.get("id")] for r in stalled):
        return outcomes.WAIT, [f"the re-run of {names} on {short} has not shown up yet."]
    return outcomes.STOP, [
        f"{outcomes.RUNNERS}: {names} on {short} was cancelled again after one re-run, with nothing failed.",
        "The branch is not known to be wrong. Once GitHub's runners are back, re-run it"
        " (`gh run rerun --failed ID`): a new run attempt re-queues it, no push needed.",
    ]


def progress(
    pull: dict, runs: list[dict], jobs: dict[int, list[dict]], files: list[str], waited: float,
    seen: frozenset[str] | set[str] = frozenset(), rerun: dict[int, int] | None = None,
    red_since: dict[tuple[int, int], float] | None = None,
) -> tuple[str, list[str]]:
    """Whether to wait, merge, or hand this branch back — and why.

    The verdict is [`verdict.decide.judge`]; this adds what a polling loop needs and
    a one-shot check does not: *not yet* against *no*. A red whose code is to
    blame stops at once, and names that run even when another workflow's red
    is still unread. A red whose jobs do not yet show the gate concluded waits
    [`GATE_SHOWS_SECONDS`] from the poll that first read it so (`red_since`,
    kept by the caller per run attempt, as `rerun` is), then stops as listed;
    any other red stops at once. A run in flight waits.
    A workflow in `seen` ([`sighted`] on an earlier poll) that is absent now
    is the listing flickering, and waits until the deadline (#592). Any other
    absence — no run, or only runs that built nothing — waits out
    [`RUN_APPEARS_SECONDS`], because the commit was pushed seconds ago and
    scorsese watched the runs listing omit a live run beside a skipped one.
    A red that is only cancelled jobs is the runners' ([`rerun_step`]).
    """
    if pull.get("isDraft"):
        return outcomes.STOP, ["the pull request is a draft.", "CI does not check drafts. Mark it ready before queueing it."]

    sha = pull.get("headRefOid", "")
    stalled = runners.outage(pull, runs, jobs)
    if stalled:
        return rerun_step(stalled, rerun or {}, sha[:7])
    unread = [(w, r) for w, r in runners.unread(pull, runs, jobs) if lagging(r, waited, red_since)]
    if unread:
        workflow, run = unread[0]
        return outcomes.WAIT, [
            f"a `{workflow}` run for {sha[:7]} is listed failed, but {runlist.lag(run, jobs, workflow)}.",
            "Whether the runners or the code reddened it is not known until the gate shows (#1109). Waiting.",
        ]
    states = [
        decide.assess(pull, w, runlist.runs_for(runs, sha, w), jobs) for w in runlist.WORKFLOWS
    ]
    failed = [(w, lines) for w, (state, lines) in zip(runlist.WORKFLOWS, states) if state == decide.FAILED]
    if failed:
        code = {w for w, _, red in runners.judged(pull, runs, jobs) if red is False}
        return outcomes.STOP, min(failed, key=lambda f: f[0] not in code)[1]
    for state, lines in states:
        if state == decide.RUNNING:
            return outcomes.WAIT, lines
    for w, (state, _) in zip(runlist.WORKFLOWS, states):
        if state == decide.ABSENT and w in seen:
            return outcomes.WAIT, [
                f"the `{w}` run seen on {sha[:7]} has dropped out of the runs listing.",
                "The listing is eventually consistent and flickers; a run seen once"
                " is not gone (#592). Waiting for it to come back.",
            ]
    if any(state in (decide.ABSENT, decide.UNBUILT) for state, _ in states):
        if waited < RUN_APPEARS_SECONDS:
            return outcomes.WAIT, [f"nothing has built {sha[:7]} yet.", "Ordinary this soon after a push, and not yet an answer."]

    ok, lines = decide.judge(pull, runs, jobs, files)
    return (outcomes.GO if ok else outcomes.STOP), lines


def lagging(run: dict, waited: float, red_since: dict[tuple[int, int], float] | None) -> bool:
    """Whether an unread red ([`verdict.runners.unread`]) is still within
    [`GATE_SHOWS_SECONDS`] of the poll that first read it, noting that poll in
    `red_since` (#1109). Without `red_since`, every poll is the first."""
    first = (red_since if red_since is not None else {}).setdefault((run.get("id"), runlist.attempt(run)), waited)
    return waited - first < GATE_SHOWS_SECONDS
