#!/usr/bin/env python3
"""`proved_tree.py` — was this push to `main`, or this PR run's merge, already proved by a run of its tree? (#1784)

Every merge to `main` re-ran the whole of `ci` (~17 minutes of Linux and macOS
runners) and `lint`, re-proving a tree the merge queue had just proved on the
pull request. It never blocked a merge, but it held runners the queue waited on:
on 2026-11-04 the run for #1711 waited up to 13 minutes for one beside such a
push run. So a gating workflow's `changes` job asks this script, on a push to
`main`, and its build steps are skipped when the answer is yes.

**Mechanical, not assumed.** The pushed commit's tree is compared with the head
tree of the workflow's recent successful runs. A squash merge of an up-to-date
head has exactly the head's tree, so a match proves the case rather than
trusting that a queue did it. A run counts only when it really built: each job
in [`BUILT`] concluded `success` and its named step ran and succeeded, so a
draft's run (gating jobs skipped) and a Markdown-only run (steps skipped) prove
nothing. A `pull_request` run tests the merge of its head into the base of that
moment, not the head; when the squash's tree equals the head's, every change on
`main` is already in the head, so that merge was the head's tree too.

**A ready pull request asks too** (#2333). A coder proves a draft by
dispatching `ci` on its branch, then readies it, and the ready run would build
the same head again. On a `pull_request` event `--sha` is the test merge commit:
its tree is what the run would build, so it is the tree compared, never the
head's. A head behind `main` merges into a tree no dispatch built, and runs in
full with no ancestry check. Only a `workflow_dispatch` or `push` run proves it
([`PROOF_EVENTS`]): their `head_commit` is the tree they built, while a
`pull_request` run's `head_commit` is its head and it built a merge. The key
check reads what the merge changes against its first parent, the base it was
made on. `superseded` is never asked: a merge commit is never `main`'s tip, so
it would skip every PR build.

**Caches keep their full run.** `Swatinem/rust-cache` keys on the toolchain,
every `Cargo.toml` / `Cargo.lock`, `.cargo/config` and the job's `CARGO*`/`RUST*`
env, and saves only on `main` (#482), only when that key has no cache yet. A
push that changes any [`CACHE_KEYED`] file since the previous tip, or the
workflow file itself, runs everything, so the new key's cache is written by
`main` and a dependency bump is proven on `main`. So does one under
`.github/actions/`: `install-tool` pins the CI tools and keys their cache (#1973). So does a change to this
script ([`DECIDER`]): its own verdict never skips its own proof. Any other push
leaves the key, and so the saved cache, as it was.

**A superseded push skips** (#1785). A merge train lands its members as one
squash each, seconds apart, so every push but the last carries a tree no run
built and `main` leaves at once. When the pushed commit is no longer `main`'s
tip (`repos/{repo}/commits/main`), its build is skipped: the newer tip's own
push run covers it. So an intermediate tree of a train is never built, and that
is sound: `main` is linear, each later tip holds every change of the one before,
nothing checks out or ships the passing tree, and the train's own run proved
the tree the last squash leaves (which then skips by the rule above, or builds).
A run that starts before the next merge sees itself as the tip and simply
builds: minutes spent, nothing less proved. The cache-key check comes first, so
a superseded push that moves the key still runs in full and saves its cache. A
partial landing leaves its last squash as the tip, and that one builds.

**Fail toward running.** Any error (an API failure, a field missing, a compare
too long to read) answers no, and the push runs every gate as before.

Writes `proved=true|false` to `$GITHUB_OUTPUT` when set, prints the reason
(with the proving run's url), and exits 0 on every answer it reaches. What it
cannot catch (no `gh`, a hung call) the workflow step absorbs: it runs with
`continue-on-error` and a short timeout, and an empty output means a full run.
Env: `GH_TOKEN`, `GITHUB_REPOSITORY`.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from typing import Callable

import ghrest
from merging.verdict.runlist import base as job_base

#: Per workflow, the jobs a run must have built in, each with the steps that do
#: the building (empty: the job's own `success` is the proof; it is skipped
#: whole when there is nothing to build). A matrix job matches by base name, and
#: its entries may split the steps between them: `build-test`'s default entry
#: runs the default suite and its two `dev` shards the `dev` one (#2264).
BUILT: dict[str, dict[str, tuple[str, ...]]] = {
    "ci.yml": {
        "build-test": ("Test (engine, default)", "Test (engine, dev features)"),
        "build-test-cross": ("Test (engine, dev features)",),
        "blender": ("Test (the .blend import, through Blender)",),
        "deny": (),
    },
    "lint.yml": {"lint": ("Clippy (hard gate, default features)",)},
}

#: What `Swatinem/rust-cache` hashes into its key, by path or file name.
CACHE_KEYED = ("Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml")
# `.github/actions/install-tool` pins the tools and keys their cache (#1973).
CACHE_KEYED_DIRS = (".cargo/", ".github/actions/")
#: This script: a change to what decides the skip is proved by a full run.
DECIDER = ".github/scripts/proved_tree.py"
#: Per event, the events whose runs may prove it, `None` for any (#2333).
PROOF_EVENTS: dict[str, tuple[str, ...] | None] = {
    "push": None,
    "pull_request": ("workflow_dispatch", "push"),
}

#: How many pages of successful runs to search; the proving run of a merge is
#: the pull request's last, minutes old.
PAGES = 2
#: The compare API lists at most this many files; a longer list is unread.
COMPARE_CAP = 300
NO_COMMIT = "0" * 40

Get = Callable[[str], object]


def gh_get(path: str):
    """`gh api <path>`, parsed; `None` on any failure."""
    done = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        return None
    try:
        return json.loads(done.stdout)
    except json.JSONDecodeError:
        return None


def keyed(path: str, workflow: str) -> bool:
    """Whether a changed path moves the cache key or the workflow itself."""
    name = path.rsplit("/", 1)[-1]
    return (name in CACHE_KEYED or path.startswith(CACHE_KEYED_DIRS)
            or path in (f".github/workflows/{workflow}", DECIDER))


def key_change(get: Get, repo: str, before: str, sha: str, workflow: str) -> str | None:
    """Why this change must run in full for its caches; `None` when it need not.
    `before` is the previous tip on a push, the merge's base on a pull request."""
    if not before or before == NO_COMMIT:
        return "no previous commit to compare with"
    compare = get(f"repos/{repo}/compare/{before}...{sha}")
    if not isinstance(compare, dict) or compare.get("status") != "ahead":
        return f"{sha} is not a plain fast-forward of {before}"
    files = compare.get("files")
    if not isinstance(files, list) or len(files) >= COMPARE_CAP:
        return "the changed files could not all be read"
    hit = sorted(f["filename"] for f in files if keyed(f.get("filename", ""), workflow))
    return f"it changes {', '.join(hit)}" if hit else None


def built(jobs: list[dict], workflow: str) -> bool:
    """Whether every job [`BUILT`] names concluded `success` having built: each
    of its entries ran one of its steps to `success`, and each step succeeded
    in one entry at least."""
    for base, steps in BUILT[workflow].items():
        mine = [j for j in jobs if job_base(j.get("name", "")) == base]
        if not mine or any(j.get("conclusion") != "success" for j in mine):
            return False
        ran = [{s.get("name") for s in j.get("steps") or [] if s.get("conclusion") == "success"} & set(steps)
               for j in mine]
        if steps and (not all(ran) or set(steps) - set().union(*ran)):
            return False
    return True


def proving_run(get: Get, repo: str, tree: str, workflow: str,
                events: tuple[str, ...] | None = None) -> str | None:
    """The url of a successful run of `workflow` that built `tree`, if one is
    recent; when `events` is given, only a run of one of those events counts."""
    for page in range(1, PAGES + 1):
        answer = get(f"repos/{repo}/actions/workflows/{workflow}/runs?status=success"
                     f"&per_page={ghrest.PER_PAGE}&page={page}")
        if not isinstance(answer, dict):
            raise LookupError("the runs list did not come")
        runs = answer["workflow_runs"]
        for run in runs:
            if (run.get("head_commit") or {}).get("tree_id") != tree:
                continue
            if events is not None and run.get("event") not in events:
                continue
            jobs = get(f"repos/{repo}/actions/runs/{run['id']}/jobs?filter=latest&per_page=100")
            if not isinstance(jobs, dict):
                raise LookupError(f"the jobs of run {run['id']} did not come")
            if built(jobs["jobs"], workflow):
                return run["html_url"]
        if len(runs) < ghrest.PER_PAGE:
            break
    return None


def superseded(get: Get, repo: str, sha: str) -> str | None:
    """`main`'s tip when it is now a commit other than `sha`; any doubt is `None`."""
    tip = get(f"repos/{repo}/commits/main")
    found = tip.get("sha") if isinstance(tip, dict) else None
    return found if isinstance(found, str) and len(found) == 40 and found != sha else None


def verdict(get: Get, repo: str, workflow: str, sha: str, before: str,
            event: str = "push") -> tuple[str | None, str]:
    """(what proves it: the proving run's url, or the newer tip that supersedes
    it; `None` when nothing does) and why, failing toward running on any error.
    On a `pull_request` event `sha` is the test merge commit and `before` is
    unused: the merge's first parent is its base."""
    try:
        commit = get(f"repos/{repo}/commits/{sha}")
        if not isinstance(commit, dict):
            return None, f"runs in full: commit {sha} could not be read"
        if event == "pull_request":
            parents = commit["parents"]
            if len(parents) != 2:
                return None, f"runs in full: {sha} is not a pull request's test merge"
            before = parents[0]["sha"]
        why = key_change(get, repo, before, sha, workflow)
        if why:
            return None, f"runs in full: {why}"
        if event == "push":
            newer = superseded(get, repo, sha)
            if newer:
                return newer, f"skips the build: `main` is at {newer} now, past {sha}, and that tip's own run covers it"
        tree = commit["commit"]["tree"]["sha"]
        url = proving_run(get, repo, tree, workflow, PROOF_EVENTS[event])
    except (KeyError, TypeError, LookupError) as error:
        return None, f"runs in full: the lookup failed ({error!r})"
    if url is None:
        events = PROOF_EVENTS[event]
        by = f"{'/'.join(events)} " if events else ""
        return None, f"runs in full: no recent successful {by}{workflow} run built tree {tree}"
    return url, f"skips the build: tree {tree} was proved by {url}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--workflow", required=True, choices=sorted(BUILT))
    parser.add_argument("--sha", required=True)
    parser.add_argument("--before", default="", help="the previous tip, on a push")
    parser.add_argument("--event", default="push", choices=sorted(PROOF_EVENTS))
    args = parser.parse_args()
    repo = os.environ.get("GITHUB_REPOSITORY", "")
    url, why = verdict(gh_get, repo, args.workflow, args.sha, args.before, args.event) if repo else (
        None, "runs in full: GITHUB_REPOSITORY is not set")
    print(f"{args.workflow} on {args.sha} {why}")
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as sink:
            sink.write(f"proved={'true' if url else 'false'}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
