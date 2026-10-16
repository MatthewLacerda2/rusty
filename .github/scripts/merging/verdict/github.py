"""The `gh` calls the verdict needs, retried through a network blip (#847).

[`gh`] parses `--json` output and retries a transient failure; [`evidence`] reads
every gating run on a commit with each run's jobs.
"""

from __future__ import annotations

import json
import subprocess
import sys
import time

from merging.verdict import runlist


# What `gh` prints when the network failed, not the request: worth asking again
# (#847: one TLS handshake timeout ended a whole overnight watch).
TRANSIENT = (
    "tls handshake timeout",
    "i/o timeout",
    "timeout awaiting",
    "connection reset",
    "connection refused",
    "unexpected eof",
    "no such host",
    "temporary failure in name resolution",
    "network is unreachable",
    "http 502",
    "http 503",
    "http 504",
)

# Seconds slept before each retry of a transient failure; about a minute in all.
RETRIES = (5, 15, 40)


class Unreachable(Exception):
    """GitHub stayed unreachable through every retry: nothing is known."""


def transient(stderr: str) -> bool:
    """Whether `gh` failed for the network's reasons rather than the request's."""
    low = stderr.lower()
    return any(sign in low for sign in TRANSIENT)


def gh(*args: str, runner=subprocess.run, sleep=time.sleep, missing: object = None) -> object:
    """`gh` with `--json`-shaped output, parsed.

    A transient failure is retried ([`RETRIES`]) and raises [`Unreachable`]
    once they run out; any other failure is fatal at once, because asking
    again would get the same answer. A caller that expects a 404 (a typo'd
    issue number) passes `missing`, returned for one instead.
    """
    for wait in (*RETRIES, None):
        done = runner(["gh", *args], capture_output=True, text=True, check=False)
        if done.returncode == 0:
            return json.loads(done.stdout)
        failure = f"gh {' '.join(args)}: {done.stderr.strip()}"
        if missing is not None and "http 404" in done.stderr.lower():
            return missing
        if not transient(done.stderr):
            sys.exit(f"mergeable: {failure}")
        if wait is None:
            raise Unreachable(failure)
        sleep(wait)
    raise AssertionError("unreachable: the loop returns or raises")


def evidence(repo: str, sha: str) -> tuple[list[dict], dict[int, list[dict]]]:
    """Every gating run on `sha`, with each run's jobs.

    Jobs are read per run rather than from `gh pr checks`, which blends runs —
    the blend is how a skipped run hides behind a real one (scorsese#245).
    """
    listed = gh("api", f"repos/{repo}/actions/runs?head_sha={sha}&per_page=100")
    runs = [r for w in runlist.WORKFLOWS for r in runlist.runs_for(listed.get("workflow_runs", []), sha, w)]
    jobs = {
        r["id"]: gh("api", f"repos/{repo}/actions/runs/{r['id']}/jobs?per_page=100").get(
            "jobs", []
        )
        for r in runs
    }
    return runs, jobs


# The pull-request fields every caller reads. `mergeable` to `statusCheckRollup`
# only matter when there is no run to judge, and make that answer specific
# (#429, #592); the rest are what the pull request claims and who it is from
# (#927, #966).
PULL_FIELDS = (
    "isDraft,headRefOid,number,files,mergeable,mergeStateStatus,statusCheckRollup,"
    "headRefName,body,closingIssuesReferences,labels,author"
)


def paths(pull: dict) -> list[str]:
    """The changed paths from `gh pr view --json files`."""
    return [f.get("path", "") for f in pull.get("files") or []]
