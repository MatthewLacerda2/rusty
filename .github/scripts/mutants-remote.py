#!/usr/bin/env python3
"""Ask GitHub's runners for a mutation run over exactly SCOPE, wait, and print it.

`make mutants-remote SCOPE=...` is this (#750). It dispatches
`mutants-on-request.yml` on the current branch, finds the run it started,
waits for it, downloads the `mutants-report` artifact and prints the report
and the survivors' diffs — so the answer is read in the terminal, not by
scrolling a log.

Why remote: every mutant rebuilds the whole crate. On a machine building
sibling worktrees that is hours of contention; on a runner it costs nothing
here, and it is the one place a cloud session may run mutation at all.

Three rules, each a way a signal learns to lie:

- **The run must exist before anything is said about it.** A dispatch returns
  no run id, so the run is found by the request id this generates, which the
  workflow puts in its run name. Not finding it is an error, never a silence
  that could read as a clean result.
- **The runner mutates what GitHub has, not what is here.** A branch whose
  local head is not the pushed head is refused before dispatch.
- **No report is never zero survivors.** A run that ends red, cancelled or
  timed out, or green without its artifact, exits 1 and says which.
  Survivors are a result: they exit 0.
"""

from __future__ import annotations

import argparse
import json
import secrets
import subprocess
import sys
import time
from pathlib import Path

WORKFLOW = "mutants-on-request.yml"

#: How long a dispatched run may take to appear in the run list.
FIND_SECONDS = 300

#: The longest wait: queueing behind pull requests' CI, plus the job's limit.
WAIT_SECONDS = 4 * 3600

#: Past this many lines the survivors' diffs are left in the file and named.
PRINT_DIFF_LINES = 400


def say(message: str) -> None:
    print(f"mutants-remote: {message}", file=sys.stderr, flush=True)


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(list(args), capture_output=True, text=True, check=False)


def gh_json(*args: str) -> object:
    done = run("gh", *args)
    if done.returncode != 0:
        sys.exit(f"mutants-remote: gh {' '.join(args)}: {done.stderr.strip()}")
    return json.loads(done.stdout)


def unpushed(branch: str, local: str, remote: str | None) -> str | None:
    """Why the runner would not be mutating this checkout, or `None` if it would."""
    if branch == "HEAD":
        return "HEAD is detached; dispatch needs a branch GitHub has"
    if remote is None:
        return f"origin/{branch} does not exist; push the branch first"
    if remote != local:
        return (f"origin/{branch} is {remote[:9]} and HEAD is {local[:9]}; the runner"
                " mutates what GitHub has, so push (or pull) first")
    return None


def find(runs: list[dict], request: str) -> dict | None:
    """The run this request started, recognised by the id in its name."""
    tag = f"[{request}]"
    return next((r for r in runs if tag in r.get("displayTitle", "")), None)


def verdict(result: dict, have_report: bool) -> tuple[bool, str]:
    """Whether there is a report to print, and the sentence that says so."""
    conclusion = result.get("conclusion") or "unfinished"
    url = result.get("url", "")
    if conclusion != "success":
        return False, (f"the run ended {conclusion}, with no report to read: {url}. That is"
                       " not a clean result; nothing about the code is known from it.")
    if not have_report:
        return False, f"the run passed but left no mutants-report artifact: {url}"
    return True, f"report from {url}"


def located(branch: str, request: str) -> dict:
    deadline = time.monotonic() + FIND_SECONDS
    while True:
        runs = gh_json("run", "list", "--workflow", WORKFLOW, "--branch", branch,
                       "--event", "workflow_dispatch", "--limit", "30",
                       "--json", "databaseId,displayTitle,url,status,conclusion")
        found = find(runs, request)
        if found is not None:
            return found
        if time.monotonic() > deadline:
            sys.exit(f"mutants-remote: dispatched request {request}, but no run carrying it"
                     f" appeared within {FIND_SECONDS}s. Nothing ran that this knows of.")
        time.sleep(10)


def finished(run_id: int, poll: int) -> dict:
    deadline, last = time.monotonic() + WAIT_SECONDS, None
    while True:
        state = gh_json("run", "view", str(run_id), "--json", "status,conclusion,url")
        if state["status"] != last:
            say(f"run {run_id} is {state['status']}")
            last = state["status"]
        if state["status"] == "completed":
            return state
        if time.monotonic() > deadline:
            sys.exit(f"mutants-remote: still {state['status']} after {WAIT_SECONDS}s: {state['url']}")
        time.sleep(poll)


def show(out: Path) -> None:
    print((out / "report.md").read_text())
    diffs = out / "survivors.diff"
    text = diffs.read_text() if diffs.is_file() else ""
    if not text.strip():
        return
    lines = text.splitlines()
    if len(lines) > PRINT_DIFF_LINES:
        print(f"\nThe survivors' diffs are {len(lines)} lines: {diffs}")
        return
    print("\n## The survivors' diffs\n")
    print(text)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="mutants-remote.py", description=__doc__.split("\n")[0])
    parser.add_argument("scope", help="`diff`, or space-separated path globs")
    parser.add_argument("--out", type=Path, default=Path("target/mutants-remote"))
    parser.add_argument("--poll", type=int, default=30, help="seconds between status checks")
    args = parser.parse_args(argv)

    branch = run("git", "rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    run("git", "fetch", "--quiet", "origin", branch)
    remote = run("git", "rev-parse", "--verify", "--quiet", f"origin/{branch}").stdout.strip()
    refusal = unpushed(branch, run("git", "rev-parse", "HEAD").stdout.strip(), remote or None)
    if refusal:
        sys.exit(f"mutants-remote: {refusal}")

    request = secrets.token_hex(4)
    done = run("gh", "workflow", "run", WORKFLOW, "--ref", branch,
               "-f", f"scope={args.scope}", "-f", f"request={request}")
    if done.returncode != 0:
        sys.exit(f"mutants-remote: dispatch refused: {done.stderr.strip()}")
    say(f"dispatched {args.scope!r} on {branch} as request {request}")
    started = located(branch, request)
    run_id = started["databaseId"]
    say(f"run {run_id}: {started['url']}")
    result = finished(run_id, args.poll)
    out = args.out / str(run_id)
    have = result.get("conclusion") == "success" and run(
        "gh", "run", "download", str(run_id), "-n", "mutants-report", "-D", str(out)
    ).returncode == 0 and (out / "report.md").is_file()
    ok, sentence = verdict(result, have)
    say(sentence)
    if not ok:
        return 1
    show(out)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
