#!/usr/bin/env python3
"""Files a CI job that ran close to its `timeout-minutes` as an issue on the board (#2276).

Three CI jobs crept up to their timeouts unnoticed (the macOS leg twice, #1976
and #2276, the Linux `build-test` once, #2264), and each time the first sign was
a cancelled run on `main`. This is the sign that comes earlier: after every
completed `ci` run on `main` (`main-health.yml`, beside `main-health.sh`), each
job's wall time is compared with its own `timeout-minutes`, read from the
workflow file the run came from, never copied here. A job that used more than
[`SHARE`] of it is told on the board.

It is an informational signal (CLAUDE.md, *Gates vs. signals*): it never fails
the `ci` run nor blocks a merge. What it cannot read (the run, its jobs, the
open issues, an issue's comments) ends it with "nothing measured" and files
nothing, so a GitHub outage never files a duplicate. A workflow file it reads
no limit from exits 1, so its step (`continue-on-error`) shows red instead of
saying every job is within its limit.

- One open issue per job, found by a hidden marker in its body, labelled
  `infrastructure` and `agent`. A matrix job is one job: its limit is the
  job's, and its slowest entry is the time told.
- A later run over the line comments only when that time moved by
  [`MOVED_MINUTES`] or more since the last note, so a steady job is told once.
- It never closes the issue: the pull request that brings the job down does.
- Only a job that ran to an end is measured (`success`, `failure`). A cancelled
  or skipped one is not: a cancelled run is the red-run path's to file.

Env: GH_TOKEN, REPO (owner/name), RUN_ID (the completed run),
DRY_RUN ('true' prints every write instead of making it).
"""

from __future__ import annotations

import base64
import json
import os
import re
import subprocess
import sys
from datetime import datetime
from typing import Callable
from urllib.parse import quote

import ghrest
from gate_mirror import _jobs

#: The workflows measured, by name: `ci` holds the gates a merge waits on.
MEASURED = ("ci",)
#: The share of a job's limit past which it is told.
SHARE = 0.75
#: How far the told time must move before a later run comments again.
MOVED_MINUTES = 1.0
#: The conclusions of a job that ran to an end, and so has a time to compare.
MEASURED_CONCLUSIONS = ("success", "failure")
LABELS = ("infrastructure", "agent")

TIMEOUT = re.compile(r"^    timeout-minutes: (?P<minutes>\d+)\b", re.M)
NOTED = re.compile(r"ci-headroom:minutes=(?P<minutes>[\d.]+)")

Gh = Callable[..., object]


class Unread(Exception):
    """A read the measure needs did not come; the run ends with nothing filed."""


class NoLimits(Exception):
    """The workflow file gave no `timeout-minutes`: the measure itself is broken."""


def limits(workflow: str) -> dict[str, int]:
    """Each job's own `timeout-minutes` (the job's key, four spaces in), by job,
    split by `gate_mirror._jobs` (comment lines dropped). A step's limit, deeper
    in, is not the job's; a job without one is left out."""
    return {name: int(limit["minutes"]) for name, body in _jobs(workflow) if (limit := TIMEOUT.search(body))}


def seconds(job: dict) -> float | None:
    """A job's wall time, from its start to its end; `None` when it has no end."""
    try:
        start, end = (datetime.fromisoformat(job[k].replace("Z", "+00:00")) for k in ("started_at", "completed_at"))
    except (KeyError, AttributeError, TypeError, ValueError):
        return None
    return (end - start).total_seconds()


def over(jobs: list[dict], limit_of: dict[str, int]) -> dict[str, tuple[str, float, int]]:
    """Each job past [`SHARE`] of its limit: its base name to (the slowest entry's
    name, its minutes, the limit). Matrix entries share their job's limit."""
    found: dict[str, tuple[str, float, int]] = {}
    for job in jobs:
        base = job.get("name", "").split(" (", 1)[0]
        took = seconds(job)
        if job.get("conclusion") not in MEASURED_CONCLUSIONS or base not in limit_of or took is None:
            continue
        minutes, limit = took / 60, limit_of[base]
        if minutes > SHARE * limit and minutes > found.get(base, ("", 0.0, 0))[1]:
            found[base] = (job["name"], minutes, limit)
    return found


def marker(base: str) -> str:
    return f"<!-- ci-headroom:job={base} -->"


def note(name: str, minutes: float, limit: int, run: dict) -> str:
    """The lines both an issue and a comment carry, the machine-read note first."""
    return (f"<!-- ci-headroom:minutes={minutes:.1f} -->\n"
            f"- Job: `{name}`, {minutes:.1f} min of its {limit}-minute `timeout-minutes` "
            f"({minutes / limit:.0%}; the line is {SHARE:.0%})\n"
            f"- Run: {run.get('html_url', '')}\n- Commit: `{run.get('head_sha', '')}` (`{run.get('event', '')}`)\n")


def opening(base: str, text: str, workflow: str) -> str:
    return (f"{marker(base)}\nThe `{workflow}` job `{base}` used more than {SHARE:.0%} of its `timeout-minutes` "
            "on `main`. Filed automatically by `ci_headroom.py` (`main-health.yml`, #2276).\n\n"
            f"{text}\n"
            "A job past its limit is a cancelled run, so this is the warning before that. Bring the job down "
            "(shard it, trim its slowest tests) rather than raising the limit, and close this from that PR: "
            f"this signal never closes it. Later runs comment here when the time moves by "
            f"{MOVED_MINUTES:g} minute or more.\n")


def last_noted(gh: Gh, repo: str, issue: dict) -> float | None:
    """The minutes of the last note on `issue`, body first, then its comments."""
    comments = ghrest.pages(lambda path: gh("api", path), f"repos/{repo}/issues/{issue['number']}/comments")
    if comments is None:
        raise Unread(f"#{issue['number']}'s comments")
    found = NOTED.findall("\n".join([issue.get("body") or ""] + [c.get("body") or "" for c in comments]))
    return float(found[-1]) if found else None


def workflow_at(gh: Gh, repo: str, run: dict) -> str:
    """The workflow file `run` ran, at its own `head_sha`, not today's `main`;
    `""` when it could not be read."""
    path = (run.get("path") or ".github/workflows/ci.yml").split("@", 1)[0]
    answer = gh("api", f"repos/{repo}/contents/{quote(path)}?ref={run.get('head_sha', '')}")
    try:
        return base64.b64decode(answer["content"]).decode("utf-8")  # type: ignore[index]
    except (KeyError, TypeError, ValueError):
        return ""


def report(gh: Gh, repo: str, run: dict, workflow_text: str, dry_run: bool = False) -> list[str]:
    """What this run files or comments, done unless `dry_run`; each action said in a line.
    A read that does not come ends it before any write; [`NoLimits`] is raised."""
    if not measured(run):
        return [f"run {run.get('id')} is '{run.get('name')}' on '{run.get('head_branch')}' via "
                f"'{run.get('event')}': not a measured main run; nothing to do"]
    try:
        return measure(gh, repo, run, workflow_text, dry_run)
    except Unread as what:
        return [f"{what} could not be read; nothing measured, nothing filed"]


def measured(run: dict) -> bool:
    """Whether `run` is one this measures: a `ci` push or schedule run on `main`."""
    return run.get("name") in MEASURED and run.get("head_branch") == "main" and run.get("event") in ("push", "schedule")


def measure(gh: Gh, repo: str, run: dict, workflow_text: str, dry_run: bool) -> list[str]:
    """[`report`]'s body: every read, then every write."""
    jobs = ghrest.pages(lambda path: gh("api", path), f"repos/{repo}/actions/runs/{run['id']}/jobs?filter=latest", "jobs")
    if jobs is None:
        raise Unread("the run's jobs")
    limit_of = limits(workflow_text)
    if not limit_of:
        raise NoLimits(f"no `timeout-minutes` read from {run.get('path')} at {run.get('head_sha')}; nothing measured")
    found = over(jobs, limit_of)
    if not found:
        return [f"every measured job is within {SHARE:.0%} of its limit"]
    issues = ghrest.pages(lambda path: gh("api", path), f"repos/{repo}/issues?state=open&labels={LABELS[0]}")
    if issues is None:
        raise Unread("the open issues")
    # Every read comes before the first write, so a read lost halfway files nothing.
    plans = []
    for base, (name, minutes, limit) in sorted(found.items()):
        open_ = next((i for i in issues if "pull_request" not in i and marker(base) in (i.get("body") or "")), None)
        plans.append((base, name, minutes, limit, open_, last_noted(gh, repo, open_) if open_ else None))
    said = []
    for base, name, minutes, limit, open_, noted in plans:
        text = note(name, minutes, limit, run)
        if open_ is None:
            payload = {"title": f"CI headroom: `{base}` ran {minutes:.0f} of its {limit} minutes on main",
                       "body": opening(base, text, run["name"]), "labels": list(LABELS)}
            path, what = f"repos/{repo}/issues", f"open an issue for `{base}`"
        else:
            if noted is not None and abs(minutes - noted) < MOVED_MINUTES:
                said.append(f"#{open_['number']} already says {noted:.1f} min for `{base}`; nothing new")
                continue
            payload = {"body": f"Still past {SHARE:.0%} of its limit:\n\n{text}"}
            path, what = f"repos/{repo}/issues/{open_['number']}/comments", f"comment on #{open_['number']}"
        said.append(f"{'DRY-RUN: ' if dry_run else ''}{what}: {name} {minutes:.1f}/{limit} min")
        if not dry_run:
            gh("api", "--method", "POST", path, "--input", "-", stdin=json.dumps(payload))
    return said


def gh_cli(*args: str, stdin: str | None = None) -> object:
    """`gh` with its JSON answer parsed; `None` when the call failed."""
    done = subprocess.run(["gh", *args], input=stdin, capture_output=True, text=True, check=False)
    if done.returncode != 0:
        print(f"ci_headroom: gh {' '.join(args[:3])}: {done.stderr.strip()}", file=sys.stderr)
        return None
    return json.loads(done.stdout) if done.stdout.strip() else None


def main() -> int:
    repo, run_id = os.environ["REPO"], os.environ["RUN_ID"]
    run = gh_cli("api", f"repos/{repo}/actions/runs/{run_id}")
    if not isinstance(run, dict):
        print(f"run {run_id} could not be read; nothing measured")
        return 0
    text = workflow_at(gh_cli, repo, run) if measured(run) else ""
    try:
        said = report(gh_cli, repo, run, text, dry_run=os.environ.get("DRY_RUN") == "true")
    except NoLimits as broken:
        # A broken measure, not a healthy run: the step fails (it is
        # `continue-on-error`, so the `ci` run and this job stay green).
        print(f"ci_headroom: {broken}", file=sys.stderr)
        return 1
    for line in said:
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
