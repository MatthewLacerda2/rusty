"""The polling loop around [`polling.progress`], until the runs on a head settle."""

from __future__ import annotations

import subprocess
import time

from merging.queue import outcomes, polling, pulls
from merging.verdict import decide, github, runlist, runners
from merging.watch import handbacks


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
                if refused:  # red for the runners, and still red: a red hand-back like any other
                    return stopped(pull, runs, jobs, [f"{outcomes.RUNNERS}, and the re-run was refused: {refused}", *lines], red=True)
            elif state == outcomes.STOP:
                return stopped(pull, runs, jobs, lines)
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


def stopped(pull: dict, runs: list[dict], jobs: dict[int, list[dict]], lines: list[str], red: bool = False):
    """A stop of the CI wait, its lines carrying the mark of the very runs the
    verdict read ([`outcomes.Marked`], #1951), never one from a later read: a
    read GitHub failed after the hand-back left #1920 with none, and the first
    read after its green re-run became the baseline. Every stop gets one, a
    missing run's included (the mark is then empty, and the run appearing
    re-admits it, #869). It is [`outcomes.RED`] when `red` (a re-run of a
    runners' red was refused) or a gating run is red ([`red_mark`])."""
    sha = pull.get("headRefOid", "")
    mark = handbacks.attempts([r for w in runlist.WORKFLOWS for r in runlist.runs_for(runs, sha, w)])
    red = red or red_mark(pull, runs, jobs) is not None
    return (outcomes.RED if red else outcomes.STOP), outcomes.Marked(lines, mark)


def red_mark(pull: dict, runs: list[dict], jobs: dict[int, list[dict]]) -> str | None:
    """The gating runs on `pull`'s head as [`handbacks.attempts`] spells them,
    when one of them is red ([`decide.assess`] says failed) and the pull request
    is not a draft; `None` otherwise (a draft, a missing run, the description).
    What makes a stop an [`outcomes.RED`] hand-back."""
    if pull.get("isDraft"):
        return None
    sha = pull.get("headRefOid", "")
    gating = {w: runlist.runs_for(runs, sha, w) for w in runlist.WORKFLOWS}
    if not any(decide.assess(pull, w, listed, jobs)[0] == decide.FAILED for w, listed in gating.items()):
        return None
    return handbacks.attempts([r for listed in gating.values() for r in listed])


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
