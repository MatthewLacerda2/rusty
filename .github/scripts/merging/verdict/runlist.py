"""Reading the runs on one commit: which count, and when each is settled.

## One commit carries several runs, and this reads all of them

scorsese#245: a draft push and a `ready_for_review` produce two runs a second
apart, and picking whichever the API lists first once refused a genuine pass —
and could as easily have passed a red one. So the rule is over the set: **no
run of a workflow for this commit failed, and at least one ran its jobs and
passed.** A red run beside a green one refuses.

**A cancelled run is the one exception, and only when superseded.** Since #482
a newer run of the same workflow on the same pull request cancels the older
one, and until #515 a cancelled run leaves a red gate beside the live green
one — on the *same* commit, when the newer run came from `ready_for_review`.
A cancelled run beside a run of its workflow on the same commit that was not
cancelled and is either still going or no older is therefore dropped
([`superseded`]); while that run is going the answer is "wait" (#562). A
cancelled run nothing replaced is still a refusal, because nothing has
answered. A `failure` is never dropped,
whatever its age.

## A run is settled when its gate is, not when its signals are

A job inside a gated run but outside its gate's `needs:` decides nothing. Until
#750 those were `mutants-pr` and `coverage-pr`, informational signals inside
the `ci` run; a mutation run could outlast the gates by far, so reading the
*run's* status held a green pull request, and the whole queue behind it, until
it ended (#555). No signal runs on a pull request now, but the rule stands. An unfinished run is judged by its gate job instead ([`by_gate`]):
once the gate concludes, its conclusion is the run's, and a signal still
running or red is named but decides nothing. A gate that has not concluded is
still a run in flight; a finished run keeps GitHub's own conclusion.
"""

from __future__ import annotations


# The workflows that gate a merge, each as (gate job, gated jobs). A gated job
# is one the gate's `needs:` collapses, `changes` aside — `changes` succeeds on
# a draft too, so it proves nothing ran. Matrix jobs report as
# `build-test-cross (macos-latest)`; [`base`] strips the suffix. Anything else
# on the commit — `main-health` (a `workflow_run` on `main`, never a PR check),
# `docs`, the coverage and mutation signals, the `windows` signal (#741: Windows
# is not a shipped platform, so its run informs and never holds a merge) — is
# not asked about, and counting it would be the same mistake in a new costume.
WORKFLOWS: dict[str, tuple[str, tuple[str, ...]]] = {
    "ci": ("ci-gate", ("build-test", "build-test-cross", "deny")),
    "lint": ("lint-gate", ("lint",)),
}


def base(job_name: str) -> str:
    """A job's name without its matrix suffix: `build-test-cross (macos-latest)`."""
    return job_name.split(" (", 1)[0]


def runs_for(runs: list[dict], sha: str, workflow: str) -> list[dict]:
    """Every run of `workflow` recorded against `sha` — all of them (scorsese#245).

    Filtered on `head_sha` and the workflow's name, never on list position.
    """
    return [r for r in runs if r.get("name") == workflow and r.get("head_sha") == sha]


def superseded(runs: list[dict]) -> list[dict]:
    """The cancelled runs another run of the same workflow replaced (#482).

    A cancelled run is dropped when a run that was *not* cancelled either is
    still going — its answer is the one to wait for — or was created no
    earlier. "No earlier" is judged on `created_at` alone, and a tie counts:
    GitHub stamps to the second, and neither the timestamp nor the run id says
    which of two same-second runs cancelled the other (#562 — on #558 the
    survivor carried the *lower* id). A cancelled run with no such partner is
    kept, and refuses: nothing has replaced its answer. Nor does one cancelled
    run ever supersede another, so a commit whose runs were all cancelled
    still refuses rather than reading as empty.
    """

    def answers_after(other: dict, run: dict) -> bool:
        if other is run or other.get("conclusion") == "cancelled":
            return False
        if other.get("status") != "completed":
            return True
        return (other.get("created_at") or "") >= (run.get("created_at") or "")

    return [
        r
        for r in runs
        if r.get("conclusion") == "cancelled" and any(answers_after(o, r) for o in runs)
    ]


def by_gate(run: dict, jobs: dict[int, list[dict]], workflow: str) -> dict:
    """`run`, settled by its gate job when the run itself has not (#555).

    A run stays `in_progress` while any of its jobs does, and a job outside
    `ci-gate`'s `needs:` (until #750, the `mutants-pr` and `coverage-pr`
    signals) shares the `ci` run. Such a job never blocks a merge
    (CLAUDE.md, *Gates vs. signals*), so an unfinished run is read as settled — status and conclusion
    both the gate's — the moment the gate concludes, and a signal still running
    or red changes nothing. The gate `needs:` every gated job, so a concluded
    gate means they concluded too.

    A run whose jobs belong to a newer attempt than the run says is read as
    that attempt, unfinished ([`latest`], #881). Otherwise only an
    *unfinished* run is read this way. A completed run keeps GitHub's
    conclusion: the signals are `continue-on-error`, so they cannot redden it,
    and a red run beside a green gate is still a refusal. A gate that has not
    concluded, or was skipped (a cancelled run), leaves the run as reported:
    running is running, absent is not passing.
    """
    run = latest(run, jobs)
    if run.get("status") == "completed":
        return run
    gate = WORKFLOWS[workflow][0]
    for j in jobs.get(run.get("id"), []):
        if j.get("name") == gate and j.get("conclusion") not in (None, "skipped"):
            return {**run, "status": "completed", "conclusion": j["conclusion"]}
    return run


def attempt(item: dict) -> int:
    """The run attempt a run or job belongs to; GitHub omits it on a first."""
    return item.get("run_attempt") or 1


def latest(run: dict, jobs: dict[int, list[dict]]) -> dict:
    """`run` as its latest attempt, when its jobs know a newer one than it does (#881).

    The runs listing can describe an older attempt than the run's own jobs
    listing, which [`github.evidence`] reads for the latest attempt alone (GitHub's
    default `filter=latest`): on 2026-10-06 the watch handed back #864 over run 37365886365's
    cancelled attempt 2, while its jobs already showed attempt 3 green. A run
    is judged by its latest attempt, so such a run is re-read as that attempt,
    unsettled: [`by_gate`] then settles it by that attempt's gate, and a gate
    still going is a run still going.
    """
    newest = max((attempt(j) for j in jobs.get(run.get("id"), [])), default=attempt(run))
    if newest <= attempt(run):
        return run
    return {**run, "run_attempt": newest, "status": "in_progress", "conclusion": None}


def by_gates(runs: list[dict], jobs: dict[int, list[dict]], workflow: str) -> list[dict]:
    """Every run of `workflow`, each read [`by_gate`]."""
    return [by_gate(r, jobs, workflow) for r in runs]


def signals(run: dict, jobs: dict[int, list[dict]], workflow: str) -> list[str]:
    """The jobs of `run` still going that its gate does not wait for (#555)."""
    gate, gated = WORKFLOWS[workflow]
    waited = {gate, "changes", *gated}
    return [
        j.get("name", "")
        for j in jobs.get(run.get("id"), [])
        if j.get("conclusion") is None and base(j.get("name", "")) not in waited
    ]


def live(runs: list[dict]) -> list[dict]:
    """The runs that still carry evidence: `runs` minus the [`superseded`]."""
    dropped = {id(r) for r in superseded(runs)}
    return [r for r in runs if id(r) not in dropped]


def failed_runs(runs: list[dict]) -> list[dict]:
    """Completed runs that concluded anything but success or skipped.

    One definition, because `merge-queue.py` polls on it too — two spellings of
    *failed* would drift, silently, in the direction that merges.
    """
    return [
        r
        for r in runs
        if r.get("status") == "completed"
        and r.get("conclusion") not in ("success", "skipped")
    ]


def unfinished(runs: list[dict]) -> list[dict]:
    """The runs that have not concluded. See [`failed_runs`]."""
    return [r for r in runs if r.get("status") != "completed"]


def lagging(runs: list[dict], jobs: dict[int, list[dict]], workflow: str) -> list[dict]:
    """The runs listed `completed` and not failed whose own jobs do not yet show
    their gate concluded (#1095).

    The runs listing and each run's jobs listing are separate reads, and either
    can be behind the other (#592 watched the runs listing flicker). On
    2026-10-20 PR #1063's `ci` run went `queued` while `ci-gate` waited for a
    runner, and the next poll handed it back as "no gated job ran"; the run
    finished green. A gate the jobs listing has not shown concluding, or not
    shown at all, is a run still going, whatever the runs listing says: only a
    concluded gate makes "nothing was built" an answer. A failed run is already
    an answer and is not asked about.
    """
    gate = WORKFLOWS[workflow][0]

    def gate_done(run: dict) -> bool:
        return any(j.get("name") == gate and j.get("conclusion") is not None for j in jobs.get(run.get("id"), []))

    settled = [r for r in runs if r.get("status") == "completed" and not failed_runs([r])]
    return [r for r in settled if not gate_done(r)]
