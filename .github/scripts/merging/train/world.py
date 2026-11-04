"""What a merge train does to the world, bound to this repository ([`effects`], the `fx` of [`watch.trains.Yard`])."""

from __future__ import annotations

import argparse
import functools
import os
import tempfile

import ghrest
from merging.queue import checks, gitio, landing, pulls, outcomes, stopping, taking
from merging.train import build, rails, ride
from merging.verdict import decide, github, runlist


def own_green(repo: str, number: int, head: str) -> bool:
    """Whether every gating workflow passed on `head` itself (#1785): the ruleset
    requires it for the merge call anyway (`strict: false`), and it keeps a
    known-red pull request out of a train. GitHub not answering is *no*."""
    try:
        runs, jobs = github.evidence(repo, head)
    except github.Unreachable:
        return False
    tip = {"number": number, "headRefOid": head}
    return all(decide.assess(tip, w, runlist.runs_for(runs, head, w), jobs)[0] == decide.PASSED for w in runlist.WORKFLOWS)


def changed_files(repo: str, number: int) -> list[str] | None:
    """The paths pull request `number` changes, `None` when GitHub failed."""
    listed = ghrest.pages(lambda path: pulls.quietly("api", path), f"repos/{repo}/pulls/{number}/files")
    return None if listed is None else [f.get("filename", "") for f in listed]


def claimed(repo: str, number: int) -> tuple[int, str, str] | None:
    """[`queue.taking.claims`], with GitHub not answering a refusal for this round."""
    try:
        return taking.claims(repo, number)
    except (github.Unreachable, SystemExit):  # SystemExit: a `gh` refusal, `verdict.github.gh`
        return number, outcomes.NOT_READY, "GitHub did not answer about its description."


def compiles(sha: str, opts: argparse.Namespace) -> list[str]:
    """Why the train's tree fails the queue's local check ([`queue.checks.verify`]),
    in a throwaway worktree; empty under `--no-check` or a dry run."""
    if opts.no_check or opts.dry_run:
        return []
    with tempfile.TemporaryDirectory(prefix="rusty-train-") as tmp:
        work = os.path.join(tmp, "wt")
        made = gitio.git("worktree", "add", "--detach", work, sha, cwd=opts.root)
        if made.returncode != 0:
            # The machine's failure, not the trees' (#580's rule, `queue.checks`):
            # a full disk must not mark every member red.
            raise stopping.Stopped([f"could not check the train out: {made.stderr.strip()}"])
        try:
            return checks.verify(work, target=checks.target_dir(opts))
        finally:
            gitio.git("worktree", "remove", "--force", work, cwd=opts.root)


def fetched(root: str) -> bool:
    """Whether `origin/main` was fetched now, through the retrying [`gitio.remote`]."""
    try:
        return gitio.remote("fetch", "--quiet", "origin", "main", cwd=root).returncode == 0
    except github.Unreachable:
        return False


def main_state(git, root: str, base: str) -> tuple[str, int] | None:
    """`origin/main`'s tree, fetched now, and its first-parent commits since
    `base`; `None` when either is unknown. A failed fetch is unknown, never the
    old `origin/main` (#1785's review: read as "nothing moved", a train would
    land on a tree CI never tested, and after a landing read as "something else
    merged", exit 9 on a healthy `main`)."""
    if not fetched(root):
        return None
    status, tree = git("rev-parse", "--verify", "--quiet", "origin/main^{tree}")
    counted, count = git("rev-list", "--first-parent", "--count", f"{base}..origin/main")
    if status != 0 or not tree.strip() or counted != 0 or not count.strip().isdigit():
        return None
    return tree.strip(), int(count.strip())


def started(git, root: str) -> build.Train | None:
    """An empty train on `origin/main`, fetched now; `None` when it could not be."""
    return build.start(git) if fetched(root) else None


def effects(repo: str, opts: argparse.Namespace, git) -> argparse.Namespace:
    """The `fx` a [`watch.trains.Yard`] runs on. `git` is the watch's own
    `(status, stdout)` git in the checkout ([`queue.world.merge_tree_git`])."""
    files: dict[str, list[str]] = {}
    green: set[str] = set()

    def listed(number: int, head: str) -> list[str] | None:
        if head not in files:
            got = changed_files(repo, number)
            if got is None:
                return None
            files[head] = got
        return files[head]

    def passed(number: int, head: str) -> bool:
        # Only a pass is remembered: a red or unfinished head is asked again.
        if head not in green and own_green(repo, number, head):
            green.add(head)
        return head in green

    return argparse.Namespace(
        start=functools.partial(started, git, opts.root),
        git=git,
        files=listed,
        green=passed,
        claims=functools.partial(claimed, repo),
        check=functools.partial(compiles, opts=opts),
        push=functools.partial(rails.push, root=opts.root),
        dispatch=functools.partial(rails.dispatch, repo),
        wait=lambda sha, tick: ride.wait(repo, sha, opts.deadline * 60, opts.poll, tick),
        delete=functools.partial(rails.delete, root=opts.root),
        look=pulls.look,
        squash=lambda pull, sha: landing.squash(repo, pull, sha, opts.root, opts.poll),
        main_state=functools.partial(main_state, git, opts.root),
        say=outcomes.say,
        bots=pulls.BOTS,
        dry_run=getattr(opts, "dry_run", False),
        no_merge=getattr(opts, "no_merge", False),
    )
