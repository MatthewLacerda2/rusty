"""Putting a branch on `main`'s tip, in a worktree nobody stands in ([`advance`]).

## What it touches, and what it leaves alone

The rebase happens in a **throwaway, detached worktree this script creates and
removes** under the system temp dir — never in a worktree somebody is working
in. The push is
`--force-with-lease` against the head the pull request had when its turn
began, so a push from anywhere else refuses rather than being overwritten.
Its `git` calls to GitHub (`fetch`, `push`) retry a network failure on
`mergeable.gh`'s backoff with that same lease ([`gitio.remote`], #875), and one
still failing after the last stops the queue as the machine's failure, never
a hand-back remembered against the branch. The
merge is `--match-head-commit`, so GitHub refuses it if the head moved after
the verdict.

A merged branch's worktree under `.claude/worktrees/` is **not** removed and
no branch is deleted — that is 12–16 GB and a checkout somebody may be
standing in. The summary names them instead.
"""

from __future__ import annotations

import os
import tempfile

from merging.queue import gitio, refusal
from merging.verdict import changes


# The commits `main` gained since a branch's base, with every path each touched.
# `--no-renames`, so a code file renamed to `.md` shows its old path too.
MAIN_GAINED = ("log", "--no-renames", "--name-only", "--format=>%H", "origin/main")

# What a head left alone by [`docs_advance`] reports instead of a push.
DOCS_ONLY = "`main` moved only by Markdown; not rebased, the run on record stands (#587)"


def conflicts(unmerged: str) -> list[str]:
    """The paths a failed rebase left conflicted, from `--diff-filter=U`."""
    return [line.strip() for line in unmerged.splitlines() if line.strip()]


def push_needed(before: str, after: str) -> bool:
    """Whether the rebase moved the head commit — and so whether to push.

    If not, the branch was already on `main`'s tip and the run on record is a
    run on this exact commit; pushing would buy a second cold run of it.
    """
    return before != after


def docs_advance(log: str) -> bool:
    """Whether `main` moved past the head, and only by [`verdict.changes.inert`] paths.

    `log` is [`MAIN_GAINED`]'s output: a `>sha` line per commit, then its paths.
    No commit is not an advance — the rebase is a no-op either way.
    """
    lines = [line.strip() for line in log.splitlines() if line.strip()]
    commits = [line for line in lines if line.startswith(">")]
    return bool(commits) and changes.inert([line for line in lines if not line.startswith(">")])


def main_moved_by_docs(head: str, root: str) -> bool:
    """[`docs_advance`] over the commits on `origin/main` that `head` lacks.

    A git failure is *no*: the queue then rebases as it always has.
    """
    done = gitio.git(*MAIN_GAINED, "--not", head, cwd=root)
    return done.returncode == 0 and docs_advance(done.stdout)


def on_tip(sha: str, root: str) -> bool:
    """Whether `sha` already contains `origin/main`'s tip (fetched by the caller)."""
    gitio.remote("fetch", "--quiet", "origin", sha, cwd=root)
    return gitio.git("merge-base", "--is-ancestor", "origin/main", sha, cwd=root).returncode == 0


def advance(
    branch: str, head: str, root: str, push: bool, check=None
) -> tuple[str | None, list[str]]:
    """Put `branch` on `main`'s tip. Returns the new head, or why not.

    In a detached worktree this creates and removes, never one an agent may be
    standing in. The push goes from inside it, leased to `head`. With
    `push=False` (the dry run) the rebase is only computed. `check`, given the
    rebased tree's path, returns why not to push it ([`checks.verify`]); it runs only
    when the rebase moved the head. A head `main` passed only by Markdown is
    returned as it is, with [`DOCS_ONLY`] as its note ([`docs_advance`]).
    """
    if main_moved_by_docs(head, root):
        return head, [f"{DOCS_ONLY}."]
    with tempfile.TemporaryDirectory(prefix="rusty-queue-") as tmp:
        work = os.path.join(tmp, "wt")
        made = gitio.git("worktree", "add", "--detach", work, head, cwd=root)
        if made.returncode != 0:
            return None, [f"could not check {branch} out: {made.stderr.strip()}"]
        try:
            done = gitio.git("rebase", "origin/main", cwd=work)
            if done.returncode != 0:
                unmerged = gitio.git("diff", "--name-only", "--diff-filter=U", cwd=work).stdout
                gitio.git("rebase", "--abort", cwd=work)
                named = conflicts(unmerged)
                return None, [
                    f"{branch} conflicts with `main`.",
                    f"Conflicted: {', '.join(named)}." if named else "The rebase stopped; git did not name a path.",
                    "Handed back unresolved: a machine that cannot say why the"
                    " code is shaped as it is does not get to pick a side.",
                ]
            fresh = gitio.git("rev-parse", "HEAD", cwd=work).stdout.strip()
            if check is not None and push_needed(head, fresh):
                broken = check(work)
                if broken:
                    return None, broken
            if push and push_needed(head, fresh):
                # Every retry carries the same lease: the head this take began
                # from, never one read back mid-retry (#875).
                pushed = gitio.remote(
                    "push", f"--force-with-lease=refs/heads/{branch}:{head}",
                    "origin", f"HEAD:refs/heads/{branch}", cwd=work,
                )
                # A push that landed before its reply was lost is refused on the
                # retry, its lease now stale against itself: GitHub already
                # holds exactly this head, so it is pushed.
                if pushed.returncode != 0:
                    now = gitio.remote_head(branch, work)
                    if now != fresh:
                        return None, refusal.explain(branch, pushed.stderr, fresh, held=now == head)
            return fresh, []
        finally:
            gitio.git("worktree", "remove", "--force", work, cwd=root)
