"""Bringing a branch up to `main`'s tip, in a worktree nobody stands in ([`advance`]).

## Merged in, never rebased (#1568)

`origin/main` is **merged into** the branch, as GitHub's *Update branch* button
does. No commit is rewritten, so the push is a fast-forward and nobody's
push in the normal flow has to rewrite history: before this the queue rebased
and force-pushed, and permission checks refused the force-pushes of coders and
the orchestrator again and again (#1551 → #1556 → #1559 → #1560 and #1554 → #1558
on 2026-10-28, each a superseding pull request with its own CI run). A merge
also resolves a conflict once, where a rebase met it once per replayed commit,
and pull requests are squash-merged, so the branch's own history never reaches
`main`.

## What it touches, and what it leaves alone

The merge happens in a **throwaway, detached worktree this script creates and
removes** under the system temp dir — never in a worktree somebody is working
in. **The push is a compare-and-swap**: `--force-with-lease=refs/heads/<branch>:<head>`,
where `<head>` is the head the pull request had when its turn began. The merge
commit descends from it, so the push is a fast-forward and the lease adds only
the expected-value check, atomically. A plain push would not do: it accepts a
branch moved *backwards* or deleted during the take (an author dropping a
leaked secret), since the merge commit fast-forwards from any ancestor, and the
dropped commit would be squash-merged into `main` (#1577's independent review).
This is the queue's own push, the one deliberate lease in the normal flow;
coders and hand-backs push without force. A refusal because the branch moved
means it moved, not that it is broken: the take ends [`refusal.MOVED`].
Its `git` calls to GitHub (`fetch`, `push`) retry a network failure on
`mergeable.gh`'s backoff ([`gitio.remote`], #875), and one still failing after
the last stops the queue as the machine's failure, never a hand-back remembered
against the branch. The merge into `main` is `--match-head-commit`, so GitHub
refuses it if the head moved after the verdict.

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
DOCS_ONLY = "`main` moved only by Markdown; not updated, the run on record stands (#587)"


def conflicts(unmerged: str) -> list[str]:
    """The paths a failed merge left conflicted, from `--diff-filter=U`."""
    return [line.strip() for line in unmerged.splitlines() if line.strip()]


def lease(branch: str, head: str) -> str:
    """The queue's push guard: refuse unless GitHub's `branch` is still `head`."""
    return f"--force-with-lease=refs/heads/{branch}:{head}"


def push_needed(before: str, after: str) -> bool:
    """Whether the update moved the head commit — and so whether to push.

    If not, the branch already contained `main`'s tip and the run on record is a
    run on this exact commit; pushing would buy a second cold run of it.
    """
    return before != after


def docs_advance(log: str) -> bool:
    """Whether `main` moved past the head, and only by [`verdict.changes.inert`] paths.

    `log` is [`MAIN_GAINED`]'s output: a `>sha` line per commit, then its paths.
    No commit is not an advance — the merge is a no-op either way.
    """
    lines = [line.strip() for line in log.splitlines() if line.strip()]
    commits = [line for line in lines if line.startswith(">")]
    return bool(commits) and changes.inert([line for line in lines if not line.startswith(">")])


def main_moved_by_docs(head: str, root: str) -> bool:
    """[`docs_advance`] over the commits on `origin/main` that `head` lacks.

    A git failure is *no*: the queue then merges `main` in as it always does.
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
    """Merge `main`'s tip into `branch`. Returns the new head, or why not.

    In a detached worktree this creates and removes, never one an agent may be
    standing in. The push goes from inside it, a fast-forward leased on `head`.
    With `push=False` (the dry run) the merge is only computed. `check`, given the
    updated tree's path, returns why not to push it ([`checks.verify`]); it runs only
    when the merge moved the head. A head `main` passed only by Markdown is
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
            # `--ff` (the flag form of `-c merge.ff=true`): a user's `merge.ff=false`
            # or `only` must not turn a no-op into a merge commit or refuse the merge.
            done = gitio.git("merge", "--ff", "--no-edit", "origin/main", cwd=work)
            if done.returncode != 0:
                unmerged = gitio.git("diff", "--name-only", "--diff-filter=U", cwd=work).stdout
                gitio.git("merge", "--abort", cwd=work)
                named = conflicts(unmerged)
                return None, [
                    f"{branch} conflicts with `main`.",
                    f"Conflicted: {', '.join(named)}." if named else f"The merge stopped; git did not name a path: {done.stderr.strip() or 'it said nothing'}",
                    "Handed back unresolved: a machine that cannot say why the"
                    " code is shaped as it is does not get to pick a side.",
                ]
            fresh = gitio.git("rev-parse", "HEAD", cwd=work).stdout.strip()
            if check is not None and push_needed(head, fresh):
                broken = check(work)
                if broken:
                    return None, broken
            if push and push_needed(head, fresh):
                # Leased on `head`: a branch moved since the take began, forward,
                # backward or deleted, is refused (see the module doc).
                pushed = gitio.remote("push", lease(branch, head), "origin", f"HEAD:refs/heads/{branch}", cwd=work)
                # A refusal while GitHub already holds exactly this head (a push
                # that landed before its reply was lost, #875) is a push.
                if pushed.returncode != 0:
                    now = gitio.remote_head(branch, work)
                    if now != fresh:
                        held = None if now is None else now == head
                        return None, refusal.explain(branch, pushed.stderr, fresh, held=held)
            return fresh, []
        finally:
            gitio.git("worktree", "remove", "--force", work, cwd=root)
