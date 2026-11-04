"""The train's branch on GitHub: push it, dispatch the gates on it, delete it, and sweep up a restart's leftovers.

`merge-train/*` branches are the queue's own throwaways: nobody else writes
them, one watch at a time holds the lock (#928), and `main-health` reads only
`main`'s runs, so a red train files nothing. Pushing one is never a force: a
branch of the same name left behind is deleted first. The gates run by
`workflow_dispatch` (`ci.yml`, `lint.yml`: both have it and run every gating job
on it, `code` defaulting to true), the same two workflows a pull request runs.
"""

from __future__ import annotations

import subprocess

from merging.queue import gitio
from merging.train import build


PREFIX = "merge-train/"

# The workflows dispatched on a train: the ones whose gates the ruleset requires
# (`verdict.runlist.WORKFLOWS`).
DISPATCHED = ("ci.yml", "lint.yml")


def branch(train: build.Train) -> str:
    """`merge-train/<main>-<a>-<b>…`: one name per (main, members), readable in the Actions list."""
    return f"{PREFIX}{train.base[:7]}-{'-'.join(str(n) for n in train.numbers)}"


def left_behind(listing: str) -> list[str]:
    """The `merge-train/*` branches in `git ls-remote` output."""
    names = [line.split("\t", 1)[1] for line in listing.splitlines() if "\t" in line]
    return [n.removeprefix("refs/heads/") for n in names if n.startswith(f"refs/heads/{PREFIX}")]


def sweep(root: str) -> list[str]:
    """Delete every `merge-train/*` branch on GitHub; the names deleted. Called
    at a watch's launch, when no train can be in flight (the lock, #928)."""
    listed = gitio.remote("ls-remote", "origin", f"refs/heads/{PREFIX}*", cwd=root)
    gone = left_behind(listed.stdout) if listed.returncode == 0 else []
    return [name for name in gone if delete(name, root)]


def delete(name: str, root: str) -> bool:
    """Delete branch `name` on GitHub; whether it is gone."""
    return gitio.remote("push", "--quiet", "origin", f":refs/heads/{name}", cwd=root).returncode == 0


def push(train: build.Train, root: str) -> str | None:
    """Push the train's tip to its branch; why not, or `None`."""
    name = branch(train)
    if gitio.remote_head(name, root) is not None:
        delete(name, root)
    done = gitio.remote("push", "--quiet", "origin", f"{train.tip}:refs/heads/{name}", cwd=root)
    return None if done.returncode == 0 else f"could not push {name}: {done.stderr.strip()}"


def dispatch(repo: str, name: str, runner=subprocess.run) -> str | None:
    """Run the gating workflows on branch `name` (REST, #970); why not, or `None`."""
    for workflow in DISPATCHED:
        done = runner(
            ["gh", "api", "--method", "POST", f"repos/{repo}/actions/workflows/{workflow}/dispatches", "-f", f"ref={name}"],
            capture_output=True, text=True, check=False,
        )
        if done.returncode != 0:
            return f"could not dispatch {workflow} on {name}: {done.stderr.strip()}"
    return None
