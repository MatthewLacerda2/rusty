"""The decision: one workflow's state ([`assess`]) and the verdict ([`judge`]).

[`judge`] is pure over plain dictionaries and the whole of the decision; the
queue polls on the same states rather than restating them.
"""

from __future__ import annotations

from merging.verdict import absent, changes, protocol, runlist


# The per-workflow states [`assess`] returns, most decisive first. A polling
# loop needs them apart: a failure is final, the rest may still change.
FAILED, RUNNING, ABSENT, UNBUILT, PASSED = (
    "failed",
    "running",
    "absent",
    "unbuilt",
    "passed",
)


def ran_something(run: dict, jobs: dict[int, list[dict]], workflow: str) -> bool:
    """Whether this run's gate passed *and* a gated job itself succeeded.

    The question `conclusion == "success"` does not answer: a draft's run
    concludes success with every gated job skipped, gate green.
    """
    gated = runlist.WORKFLOWS[workflow][1]
    ran = jobs.get(run.get("id"), [])
    gate_ok = runlist.gate_conclusion(run, jobs, workflow) == "success"
    built = any(
        runlist.base(j.get("name", "")) in gated and j.get("conclusion") == "success" for j in ran
    )
    return gate_ok and built


def assess(
    pull: dict, workflow: str, runs: list[dict], jobs: dict[int, list[dict]]
) -> tuple[str, list[str]]:
    """One workflow's state on the head commit, and the words for it.

    `runs` is every run of `workflow` on the head (see [`runlist.runs_for`]). Order is
    [`FAILED`] → [`RUNNING`] → [`ABSENT`] / [`UNBUILT`] → [`PASSED`]: a commit
    carrying a red run and a live one is already answered. A run whose gate
    has not concluded is [`RUNNING`] whatever the runs listing says of it
    ([`runlist.lagging`], #1095): [`UNBUILT`] is judged only on a settled gate.
    """
    short = pull.get("headRefOid", "")[:7]
    if not runs:
        return ABSENT, absent.no_run(pull, short, workflow)

    runs = runlist.by_gates(runs, jobs, workflow)

    kept = runlist.live(runs)
    failed = runlist.failed_runs(kept)
    if failed:
        run = failed[0]
        lines = [
            f"a `{workflow}` run for {short} concluded {run.get('conclusion')}.",
            f"See {run.get('html_url', 'the run')}.",
        ]
        if len(kept) > 1:
            lines.append(
                f"{len(kept)} `{workflow}` runs exist for this commit; a green one"
                " beside a red one is not a pass."
            )
        return FAILED, lines

    running = runlist.unfinished(kept)
    if running:
        return RUNNING, [
            f"a `{workflow}` run for {short} is {running[0].get('status')}.",
            "Wait for it. A run in flight has not passed yet.",
        ]
    lagging = runlist.lagging(kept, jobs, workflow)
    if lagging:
        return RUNNING, [
            f"a `{workflow}` run for {short} is listed completed, but {runlist.lag(lagging[0], jobs, workflow)}.",
            "The two listings are read apart and one can lag the other (#1095). Wait for the gate.",
        ]

    real = [r for r in kept if ran_something(r, jobs, workflow)]
    if not real:
        gate = runlist.WORKFLOWS[workflow][0]
        return UNBUILT, [
            f"no `{workflow}` run for {short} ran a gated job.",
            f"`{gate}` can pass with every gated job skipped — that is what a"
            " run against a draft looks like — so this is a run created before"
            " the pull request was ready and never replaced (scorsese#153)."
            " Nothing checked this commit.",
        ]

    run = real[0]
    ran = jobs.get(run.get("id"), [])
    passed = [j for j in ran if j.get("conclusion") == "success"]
    lines = [f"`{workflow}` passed on {short}: {len(passed)} jobs succeeded."]
    skipped = [j["name"] for j in ran if j.get("conclusion") == "skipped"]
    if skipped:
        # Naming what did not run is the difference between this and the
        # report it was written to distrust.
        lines.append(f"Skipped: {', '.join(skipped)}.")
    going = runlist.signals(run, jobs, workflow)
    if going:
        lines.append(f"Still running, and not gating: {', '.join(going)}.")
    if len(runs) > 1:
        # Said out loud: "which run answered" is what scorsese#245 was about.
        dropped = len(runs) - len(kept)
        why = f" ({dropped} cancelled and superseded)" if dropped else ""
        lines.append(
            f"{len(runs)} `{workflow}` runs exist for this commit{why}; none"
            " failed, and this is the one that checked it."
        )
    return PASSED, lines


def judge(
    pull: dict, runs: list[dict], jobs: dict[int, list[dict]], files: list[str]
) -> tuple[bool, list[str]]:
    """Whether this pull request may be merged, and why not when it may not.

    Pure, and the whole of the decision. `pull` is [`github.pull`], `runs`
    every workflow run on the head commit (any workflow — this filters),
    `jobs` maps a run's id to its jobs (with steps), `files` is the changed
    paths. What the pull request claims is judged before any run
    ([`protocol.refusal`]): it is wrong whatever CI says.
    """
    sha = pull.get("headRefOid", "")
    if pull.get("isDraft"):
        return False, [
            "the pull request is a draft.",
            "CI does not check drafts, so nothing here has checked it. A draft"
            " makes no claim to pass; mark it ready and let CI answer.",
        ]
    refused = protocol.refusal(pull)
    if refused:
        return False, refused

    states = {w: assess(pull, w, runlist.runs_for(runs, sha, w), jobs) for w in runlist.WORKFLOWS}
    for worst in (FAILED, RUNNING, ABSENT, UNBUILT):
        for state, lines in states.values():
            if state == worst:
                return False, lines

    lines = [line for _, said in states.values() for line in said]
    if changes.needs_drift_run(files):
        ci = [r for r in runlist.live(runlist.by_gates(runlist.runs_for(runs, sha, "ci"), jobs, "ci")) if ran_something(r, jobs, "ci")]
        if not any(changes.drift_ran(r, jobs) for r in ci):
            return False, [
                f"`ci` passed on {sha[:7]}, but it never ran the doc-drift tests.",
                "Every change here is Markdown, so CI skipped its build steps —"
                " and one of them is under `docs/api/`, which"
                " `tests/api_doc_drift.rs` and `tests/callback_doc_drift.rs`"
                " parse. CLAUDE.md treats that directory as code; CI's filter does"
                " not yet (#525).",
                "Run `cargo test --features dev --test api_doc_drift --test"
                " callback_doc_drift` and push any non-Markdown change alongside"
                " it, or land #525 first.",
                *lines,
            ]
    elif changes.markdown_only(files):
        lines.append("Markdown-only: CI's jobs ran with their build steps skipped, by design.")
    return True, lines + protocol.gates(pull)
