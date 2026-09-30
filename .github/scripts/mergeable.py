#!/usr/bin/env python3
"""`mergeable.py N` — say whether pull request N has really been checked.

Exits 0 when CI genuinely ran on the head commit and passed, and non-zero with
a reason otherwise. It is the last thing the merge routine does before
`gh pr merge`, and it exists because "the checks look green" and "the checks
ran" are not the same claim. Ported from scorsese (#486), whose incidents the
reasoning below cites as scorsese#N.

**On rusty it is the only thing standing between a red pull request and
`main`.** `main` has no required status checks (#491 is the human task that
would add them), so GitHub will merge a pull request whose CI is red, running,
or absent. Every answer here that is not an explicit pass is a refusal, and an
absent check is never read as a green one.

## Two workflows, and why neither's green is the other's

A pull request here is checked by two workflows, `ci` (`ci.yml`) and `lint`
(`lint.yml`). Each collapses its gating jobs into one verdict job — `ci-gate`
and `lint-gate` — and **both must pass on the head commit**, because they
check different things: `ci` builds and tests, `lint` runs clippy, rustdoc and
the tools/lint gates. A green `ci` beside a missing `lint` is half a check.

**A gate job that passed is not yet evidence anything ran.** Both gates run
unless the run was cancelled (`!cancelled()`, #515) and pass when their gated jobs *succeeded or were skipped*. On a
draft every gated job skips by design — so a draft's run concludes success,
gate green, with nothing compiled. That is scorsese#153's shape: push to a
draft, mark it ready seconds later, and the `ready_for_review` run sometimes
never appears, leaving a ready pull request whose only run is the draft's. So a
run counts only when its gate passed **and** at least one gated job itself
concluded success ([`ran_something`]). [`WORKFLOWS`] names the gated jobs, and a
test holds that list to each gate's `needs:` in the workflow files.

A Markdown-only pull request passes that test honestly: `build-test` and `lint`
still *run*, with their steps skipped, and report success (#218 is why the jobs
run rather than skip). Nothing compiled, by design, and the verdict says so.

**Except `docs/scripting-api.md`.** Two hard-gate tests parse it, so CLAUDE.md
treats a pull request touching it as code. CI's `code` filter does not (#525):
a pull request whose changes are all Markdown skips every step, drift tests
included. So when such a pull request touches that file, the pass additionally
needs the `build-test` step that runs the dev-feature tests to have concluded
success ([`drift_ran`]) — which today it cannot, and the refusal says why.

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
A cancelled run with a newer run of its workflow on the same commit is
therefore dropped ([`superseded`]); a cancelled run that *is* the newest is
still a refusal, because nothing has answered. A `failure` is never dropped,
whatever its age.

## A run is settled when its gate is, not when its signals are

`mutants-pr` and `coverage-pr` run inside the `ci` run but outside
`ci-gate`'s `needs:` — informational signals that never block a merge. A
mutation run can outlast the gates by far, so reading the *run's* status would
hold a green pull request, and the whole queue behind it, until it ends
(#555). An unfinished run is judged by its gate job instead ([`by_gate`]):
once the gate concludes, its conclusion is the run's, and a signal still
running or red is named but decides nothing. A gate that has not concluded is
still a run in flight; a finished run keeps GitHub's own conclusion.

## A commit with no run at all

Two causes, told apart in the output ([`no_run`]): scorsese#153 above, and a
branch that conflicts with `main` — GitHub cannot compute a merge ref, so it
creates no run, no check and no error (scorsese#429, 45 minutes lost to editing
workflow YAML). An invalid workflow *does* produce a run, a `startup_failure`;
no run whatsoever is a conflict.

Branch protection is not the alternative it looks like: GitHub treats a skipped
required check as satisfied in some configurations, and a draft's run is
exactly that — this asks the question directly.

Run it:

    make mergeable PR=524
    python3 .github/scripts/mergeable.py 524

`merge-queue.py` beside this file asks the same question in a loop and imports
this decision rather than restating it. The decision is [`judge`], pure over
plain dictionaries, so all of it is tested without a network
(`make scripts`). Python, stdlib only, because it is a few `gh` calls and a
decision — and `.github/scripts/` is where that lives.
"""

from __future__ import annotations

import json
import subprocess
import sys

# The workflows that gate a merge, each as (gate job, gated jobs). A gated job
# is one the gate's `needs:` collapses, `changes` aside — `changes` succeeds on
# a draft too, so it proves nothing ran. Matrix jobs report as
# `build-test-cross (macos-latest)`; [`base`] strips the suffix. Anything else
# on the commit — `main-health` (a `workflow_run` on `main`, never a PR check),
# `docs`, the coverage and mutation signals — is not asked about, and counting
# it would be the same mistake in a new costume.
WORKFLOWS: dict[str, tuple[str, tuple[str, ...]]] = {
    "ci": ("ci-gate", ("build-test", "build-test-cross", "deny")),
    "lint": ("lint-gate", ("lint",)),
}

# Markdown that is code for merge purposes: tests parse it (CLAUDE.md).
CODE_DOCS = ("docs/scripting-api.md",)

# Where the doc-drift tests run: a `build-test` step whose name says it runs
# the tests with the dev features (`Test (engine, dev features)` today). Matched
# on words rather than the exact name so a rename that keeps the meaning
# (#484's nextest) keeps working; a test holds ci.yml to it.
DRIFT_JOB = "build-test"
DRIFT_STEP_WORDS = ("test", "dev")

# What GitHub calls a branch it cannot merge, in the two fields that say so.
# Both are read, because either alone has been seen to lag the other.
CONFLICTED = ("CONFLICTING", "DIRTY")

# Not "no": *not computed yet*. A fresh push reads UNKNOWN for a moment, and
# asserting "not conflicted" from it is a confident wrong answer.
UNKNOWN = "UNKNOWN"

# The per-workflow states [`assess`] returns, most decisive first. A polling
# loop needs them apart: a failure is final, the rest may still change.
FAILED, RUNNING, ABSENT, UNBUILT, PASSED = (
    "failed",
    "running",
    "absent",
    "unbuilt",
    "passed",
)


def gh(*args: str) -> object:
    """`gh` with `--json`-shaped output, parsed. Fatal if `gh` itself fails."""
    done = subprocess.run(["gh", *args], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        sys.exit(f"mergeable: gh {' '.join(args)}: {done.stderr.strip()}")
    return json.loads(done.stdout)


def base(job_name: str) -> str:
    """A job's name without its matrix suffix: `build-test-cross (macos-latest)`."""
    return job_name.split(" (", 1)[0]


def runs_for(runs: list[dict], sha: str, workflow: str) -> list[dict]:
    """Every run of `workflow` recorded against `sha` — all of them (scorsese#245).

    Filtered on `head_sha` and the workflow's name, never on list position.
    """
    return [r for r in runs if r.get("name") == workflow and r.get("head_sha") == sha]


def superseded(runs: list[dict]) -> list[dict]:
    """The cancelled runs a newer run of the same workflow replaced (#482).

    Newer by `created_at`, then `id` — both only ever grow. A cancelled run
    that is itself the newest is kept: nothing has replaced its answer.
    """

    def age(run: dict) -> tuple[str, int]:
        return (run.get("created_at") or "", run.get("id") or 0)

    newest = max((age(r) for r in runs), default=None)
    return [r for r in runs if r.get("conclusion") == "cancelled" and age(r) != newest]


def by_gate(run: dict, jobs: dict[int, list[dict]], workflow: str) -> dict:
    """`run`, settled by its gate job when the run itself has not (#555).

    A run stays `in_progress` while any of its jobs does, and the signal jobs
    (`mutants-pr`, `coverage-pr`) share the `ci` run without being in
    `ci-gate`'s `needs:`. They never block a merge (CLAUDE.md, *Gates vs.
    signals*), so an unfinished run is read as settled — status and conclusion
    both the gate's — the moment the gate concludes, and a signal still running
    or red changes nothing. The gate `needs:` every gated job, so a concluded
    gate means they concluded too.

    Only an *unfinished* run is read this way. A completed run keeps GitHub's
    conclusion: the signals are `continue-on-error`, so they cannot redden it,
    and a red run beside a green gate is still a refusal. A gate that has not
    concluded, or was skipped (a cancelled run), leaves the run as reported:
    running is running, absent is not passing.
    """
    if run.get("status") == "completed":
        return run
    gate = WORKFLOWS[workflow][0]
    for j in jobs.get(run.get("id"), []):
        if j.get("name") == gate and j.get("conclusion") not in (None, "skipped"):
            return {**run, "status": "completed", "conclusion": j["conclusion"]}
    return run


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


def ran_something(run: dict, jobs: dict[int, list[dict]], workflow: str) -> bool:
    """Whether this run's gate passed *and* a gated job itself succeeded.

    The question `conclusion == "success"` does not answer: a draft's run
    concludes success with every gated job skipped, gate green.
    """
    gate, gated = WORKFLOWS[workflow]
    ran = jobs.get(run.get("id"), [])
    gate_ok = any(j.get("name") == gate and j.get("conclusion") == "success" for j in ran)
    built = any(
        base(j.get("name", "")) in gated and j.get("conclusion") == "success" for j in ran
    )
    return gate_ok and built


def markdown_only(files: list[str]) -> bool:
    """Whether every changed path is Markdown — CI's own `code` test."""
    return bool(files) and all(f.endswith(".md") for f in files)


def needs_drift_run(files: list[str]) -> bool:
    """A Markdown-only change that touches a doc tests parse (#525)."""
    return markdown_only(files) and any(f in CODE_DOCS for f in files)


def drift_ran(run: dict, jobs: dict[int, list[dict]]) -> bool:
    """Whether this `ci` run's `build-test` really ran the dev-feature tests."""
    for job in jobs.get(run.get("id"), []):
        if job.get("name") != DRIFT_JOB:
            continue
        for step in job.get("steps") or []:
            name = step.get("name", "").lower()
            if all(w in name for w in DRIFT_STEP_WORDS) and step.get("conclusion") == "success":
                return True
    return False


def no_run(pull: dict, short: str, workflow: str) -> list[str]:
    """Why this commit has no `workflow` run at all — scorsese#153 or #429.

    Told apart here, because the reader is already looking at this output.
    """
    state, status = pull.get("mergeable"), pull.get("mergeStateStatus")
    head = f"no `{workflow}` run exists for the head commit {short}."

    if state in CONFLICTED or status in CONFLICTED:
        return [
            f"{head} The branch conflicts with `main`.",
            "That is the cause and not a coincidence: GitHub cannot compute a"
            " merge ref for a conflicted branch, so it never evaluates the"
            " workflow's triggers and creates no run, no check and no error"
            " anywhere. The workflow file is fine — do not edit it.",
            "Rebase onto `main` and force-push; the merge routine asks for that"
            " rebase anyway.",
        ]

    lines = [
        head,
        "Marking a pull request ready right after a push can lose the run"
        " entirely (scorsese#153). The checks are not green, they are absent.",
        "Force one with an empty commit, or draft and ready it again with a"
        " pause in between.",
        "Before editing any workflow YAML: an invalid one still produces a run,"
        " a `startup_failure`. No run whatsoever means a conflict with `main`,"
        " not a syntax error.",
    ]
    if state == UNKNOWN:
        # Said out loud: the conflict branch above could not fire, and silence
        # would read as "checked, and it is not that".
        lines.append(
            "GitHub has not finished computing whether this branch merges"
            " cleanly. Ask again in a moment before believing the rest."
        )
    elif status and status != "CLEAN":
        lines.append(f"GitHub reports this branch as {status}.")
    return lines


def assess(
    pull: dict, workflow: str, runs: list[dict], jobs: dict[int, list[dict]]
) -> tuple[str, list[str]]:
    """One workflow's state on the head commit, and the words for it.

    `runs` is every run of `workflow` on the head (see [`runs_for`]). Order is
    [`FAILED`] → [`RUNNING`] → [`ABSENT`] / [`UNBUILT`] → [`PASSED`]: a commit
    carrying a red run and a live one is already answered.
    """
    short = pull.get("headRefOid", "")[:7]
    if not runs:
        return ABSENT, no_run(pull, short, workflow)

    runs = by_gates(runs, jobs, workflow)

    kept = live(runs)
    failed = failed_runs(kept)
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

    running = unfinished(kept)
    if running:
        return RUNNING, [
            f"a `{workflow}` run for {short} is {running[0].get('status')}.",
            "Wait for it. A run in flight has not passed yet.",
        ]

    real = [r for r in kept if ran_something(r, jobs, workflow)]
    if not real:
        gate = WORKFLOWS[workflow][0]
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
    going = signals(run, jobs, workflow)
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

    Pure, and the whole of the decision. `pull` is `gh pr view --json`, `runs`
    every workflow run on the head commit (any workflow — this filters),
    `jobs` maps a run's id to its jobs (with steps), `files` is the changed
    paths.
    """
    sha = pull.get("headRefOid", "")
    if pull.get("isDraft"):
        return False, [
            "the pull request is a draft.",
            "CI does not check drafts, so nothing here has checked it. A draft"
            " makes no claim to pass; mark it ready and let CI answer.",
        ]

    states = {w: assess(pull, w, runs_for(runs, sha, w), jobs) for w in WORKFLOWS}
    for worst in (FAILED, RUNNING, ABSENT, UNBUILT):
        for state, lines in states.values():
            if state == worst:
                return False, lines

    lines = [line for _, said in states.values() for line in said]
    if needs_drift_run(files):
        ci = [r for r in live(by_gates(runs_for(runs, sha, "ci"), jobs, "ci")) if ran_something(r, jobs, "ci")]
        if not any(drift_ran(r, jobs) for r in ci):
            return False, [
                f"`ci` passed on {sha[:7]}, but it never ran the doc-drift tests.",
                "Every change here is Markdown, so CI skipped its build steps —"
                " and one of them is `docs/scripting-api.md`, which"
                " `tests/api_doc_drift.rs` and `tests/callback_doc_drift.rs`"
                " parse. CLAUDE.md treats that file as code; CI's filter does"
                " not yet (#525).",
                "Run `cargo test --features dev --test api_doc_drift --test"
                " callback_doc_drift` and push any non-Markdown change alongside"
                " it, or land #525 first.",
                *lines,
            ]
    elif markdown_only(files):
        lines.append("Markdown-only: CI's jobs ran with their build steps skipped, by design.")
    return True, lines


def evidence(repo: str, sha: str) -> tuple[list[dict], dict[int, list[dict]]]:
    """Every gating run on `sha`, with each run's jobs.

    Jobs are read per run rather than from `gh pr checks`, which blends runs —
    the blend is how a skipped run hides behind a real one (scorsese#245).
    """
    listed = gh("api", f"repos/{repo}/actions/runs?head_sha={sha}&per_page=100")
    runs = [r for w in WORKFLOWS for r in runs_for(listed.get("workflow_runs", []), sha, w)]
    jobs = {
        r["id"]: gh("api", f"repos/{repo}/actions/runs/{r['id']}/jobs?per_page=100").get(
            "jobs", []
        )
        for r in runs
    }
    return runs, jobs


# The pull-request fields every caller reads. The last two only matter when
# there is no run to judge, and they make that answer specific (#429).
PULL_FIELDS = "isDraft,headRefOid,number,mergeable,mergeStateStatus,files"


def paths(pull: dict) -> list[str]:
    """The changed paths from `gh pr view --json files`."""
    return [f.get("path", "") for f in pull.get("files") or []]


def main() -> int:
    if len(sys.argv) != 2:
        sys.exit("usage: mergeable.py PULL_REQUEST_NUMBER")
    number = sys.argv[1]

    repo = gh("repo", "view", "--json", "nameWithOwner")["nameWithOwner"]
    pull = gh("pr", "view", number, "--json", PULL_FIELDS)
    runs, jobs = evidence(repo, pull["headRefOid"])

    ok, lines = judge(pull, runs, jobs, paths(pull))
    head, *rest = lines
    where = sys.stdout if ok else sys.stderr
    print(f"mergeable: #{number}: {head}", file=where)
    for line in rest:
        print(f"  {line}", file=where)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
