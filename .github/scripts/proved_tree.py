#!/usr/bin/env python3
"""`proved_tree.py` — was this push to `main` already proved by a run of its tree? (#1784)

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

**Caches keep their full run.** `Swatinem/rust-cache` keys on the toolchain,
every `Cargo.toml` / `Cargo.lock`, `.cargo/config` and the job's `CARGO*`/`RUST*`
env, and saves only on `main` (#482), only when that key has no cache yet. A
push that changes any [`CACHE_KEYED`] file since the previous tip, or the
workflow file itself, runs everything, so the new key's cache is written by
`main` and a dependency bump is proven on `main`. Any other push leaves the key,
and so the saved cache, as it was.

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

#: Per workflow, the jobs a run must have built in, each with the step that does
#: the building (`None`: the job's own `success` is the proof; it is skipped
#: whole when there is nothing to build). A matrix job matches by base name.
BUILT: dict[str, dict[str, str | None]] = {
    "ci.yml": {
        "build-test": "Test (engine, default)",
        "build-test-cross": "Test (engine, default)",
        "blender": "Test (the .blend import, through Blender)",
        "deny": None,
    },
    "lint.yml": {"lint": "Clippy (hard gate, default features)"},
}

#: What `Swatinem/rust-cache` hashes into its key, by path or file name.
CACHE_KEYED = ("Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml")
CACHE_KEYED_DIRS = (".cargo/",)

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
            or path == f".github/workflows/{workflow}")


def key_change(get: Get, repo: str, before: str, sha: str, workflow: str) -> str | None:
    """Why this push must run in full for its caches; `None` when it need not."""
    if not before or before == NO_COMMIT:
        return "no previous tip to compare with"
    compare = get(f"repos/{repo}/compare/{before}...{sha}")
    if not isinstance(compare, dict) or compare.get("status") != "ahead":
        return "the push is not a plain fast-forward of the previous tip"
    files = compare.get("files")
    if not isinstance(files, list) or len(files) >= COMPARE_CAP:
        return "the changed files could not all be read"
    hit = sorted(f["filename"] for f in files if keyed(f.get("filename", ""), workflow))
    return f"it changes {', '.join(hit)}" if hit else None


def built(jobs: list[dict], workflow: str) -> bool:
    """Whether every job [`BUILT`] names concluded `success` with its step run."""
    for base, step in BUILT[workflow].items():
        mine = [j for j in jobs if j.get("name", "").split(" (", 1)[0] == base]
        if not mine or any(j.get("conclusion") != "success" for j in mine):
            return False
        if step and not all(any(s.get("name") == step and s.get("conclusion") == "success"
                                for s in j.get("steps") or []) for j in mine):
            return False
    return True


def proving_run(get: Get, repo: str, tree: str, workflow: str) -> str | None:
    """The url of a successful run of `workflow` that built `tree`, if one is recent."""
    for page in range(1, PAGES + 1):
        answer = get(f"repos/{repo}/actions/workflows/{workflow}/runs?status=success"
                     f"&per_page={ghrest.PER_PAGE}&page={page}")
        if not isinstance(answer, dict):
            raise LookupError("the runs list did not come")
        runs = answer["workflow_runs"]
        for run in runs:
            if (run.get("head_commit") or {}).get("tree_id") != tree:
                continue
            jobs = get(f"repos/{repo}/actions/runs/{run['id']}/jobs?filter=latest&per_page=100")
            if not isinstance(jobs, dict):
                raise LookupError(f"the jobs of run {run['id']} did not come")
            if built(jobs["jobs"], workflow):
                return run["html_url"]
        if len(runs) < ghrest.PER_PAGE:
            break
    return None


def verdict(get: Get, repo: str, workflow: str, sha: str, before: str) -> tuple[str | None, str]:
    """(the proving run's url or `None`, why), failing toward running on any error."""
    try:
        why = key_change(get, repo, before, sha, workflow)
        if why:
            return None, f"runs in full: {why}"
        commit = get(f"repos/{repo}/commits/{sha}")
        if not isinstance(commit, dict):
            return None, "runs in full: the pushed commit could not be read"
        tree = commit["commit"]["tree"]["sha"]
        url = proving_run(get, repo, tree, workflow)
    except (KeyError, TypeError, LookupError) as error:
        return None, f"runs in full: the lookup failed ({error!r})"
    if url is None:
        return None, f"runs in full: no recent successful {workflow} run built tree {tree}"
    return url, f"skips the build: tree {tree} was proved by {url}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--workflow", required=True, choices=sorted(BUILT))
    parser.add_argument("--sha", required=True)
    parser.add_argument("--before", default="")
    args = parser.parse_args()
    repo = os.environ.get("GITHUB_REPOSITORY", "")
    url, why = verdict(gh_get, repo, args.workflow, args.sha, args.before) if repo else (
        None, "runs in full: GITHUB_REPOSITORY is not set")
    print(f"{args.workflow} on {args.sha} {why}")
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as sink:
            sink.write(f"proved={'true' if url else 'false'}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
