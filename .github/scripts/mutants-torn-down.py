#!/usr/bin/env python3
"""Say why an on-request mutation job left no answer, from outside it (#2230).

`mutants-on-request.yml`'s own `Say why there is no answer` step covers a job
whose steps ran. It cannot cover a job that was torn down: a runner that died
(out of memory, out of disk, "lost communication with the server") or a job
killed at its `timeout-minutes` runs no later step at all, so no report is
written and nothing reaches the pull request. The workflow's `report` job,
which `needs:` the others, runs this for the `plan` job or a shard (`mutants (K)`)
that left nothing, and says its line instead (#2283).

    mutants-torn-down.py --repo o/r --run 123 --job mutants

It prints one sentence for `mutants-report.py --unanswered`: the step the job
was in when it ended, and what GitHub said about it (the job's failure
annotations: "The hosted runner lost communication with the server.", "The
job has exceeded the maximum execution time of …"), so the reader can tell a
runner that died from a job that ran out of time without opening the log.
The job also fires when the steps ran and still wrote no report (the checkout
of the commit failed, the report script crashed); the sentence then says the
job failed with its steps run, not that it was torn down (`torn_down`).

What the job measured before it died is not recoverable here: its files lived
on the runner that went with it, so the line says the ask is unanswered.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys

import ghrest

#: GitHub's own line on every interrupted step; it names no cause.
GENERIC = ("The operation was canceled.",)

#: Words of GitHub's annotations on a job whose runner went away.
RUNNER_GONE = ("lost communication", "shutdown signal")

#: Step conclusions of a step a torn-down job left: never finished, or interrupted.
UNFINISHED = (None, "cancelled")


def last_step(job: dict) -> str | None:
    """The step the job was in when it ended: the first one it left unfinished
    or `cancelled` (where a torn-down job stops), else the first that did not
    finish as a success or a skip; `None` when every step finished."""
    steps = job.get("steps", [])
    for wanted in (lambda c: c in UNFINISHED, lambda c: c not in ("success", "skipped")):
        for step in steps:
            if wanted(step.get("conclusion")):
                return step.get("name")
    return None


def causes(annotations: list[dict]) -> list[str]:
    """GitHub's failure messages on the job, the uninformative one dropped."""
    return [a["message"].strip() for a in annotations
            if a.get("annotation_level") == "failure" and a.get("message", "").strip() not in GENERIC]


def torn_down(job: dict, annotations: list[dict]) -> bool:
    """Whether the job was torn down rather than failing with its steps run: a
    step left unfinished or `cancelled` (a dead runner, a job past its
    `timeout-minutes`), or GitHub saying the runner went away."""
    if any(step.get("conclusion") in UNFINISHED for step in job.get("steps", [])):
        return True
    said = " ".join(causes(annotations)).lower()
    return any(words in said for words in RUNNER_GONE)


def reason(job: dict, annotations: list[dict]) -> str:
    """The sentence for the pull request."""
    where = last_step(job)
    said = causes(annotations)
    step = f" during its `{where}` step" if where else ""
    github = f" (GitHub: {' '.join(said)})" if said else ""
    if torn_down(job, annotations):
        return (f"the mutation job was torn down{step}{github}, so no step was left to write a report; "
                "what it measured died with its runner. "
                "A mutant that grows without bound or a job past its time limit does this.")
    return (f"the mutation job failed{step}{github} with its steps run, and no report was written; "
            "its log says why.")


def gh(*args: str) -> object:
    done = subprocess.run(["gh", "api", *args], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        sys.exit(f"mutants-torn-down: gh api {args[0]}: {done.stderr.strip()}")
    return json.loads(done.stdout)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="mutants-torn-down.py", description=__doc__.split("\n")[0])
    parser.add_argument("--repo", required=True, help="owner/name")
    parser.add_argument("--run", required=True, help="the workflow run's id")
    parser.add_argument("--job", required=True, help="the name of the job that left no answer")
    args = parser.parse_args(argv)
    jobs = gh(f"repos/{args.repo}/actions/runs/{args.run}/jobs?per_page=100")["jobs"]
    job = next((j for j in jobs if j.get("name") == args.job), None)
    if job is None:
        print(f"the {args.job} job left no report, and the run lists no such job.")
        return 0
    annotations = ghrest.pages(gh, f"repos/{args.repo}/check-runs/{job['id']}/annotations") or []
    print(reason(job, annotations))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
