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

## A conflict

A merge that stops is handed back naming the paths, with one exception (#1723):
when every conflicted file is a `sorted` list both sides only added to, the
conflicted files are rewritten in `diff3` style, one at a time ([`diff3`]),
resolved by keeping both sides' entries in key order ([`conflict.resolve`]),
committed, and taken on exactly as a clean merge is: the check, the leased push,
CI. Both sides' lists being sorted makes the union sorted; the `sorted` gate
checks it where it runs, which is not every pull request (`lint.yml` skips it on
a Markdown-only one). One file outside the rule hands the whole merge back.
Each conflict is recorded ([`conflict.record`]) through the `log` the take
passes, once its outcome is known: `resolved` only when the check passed and
the push landed, `handed back` on every other way out. A take stopped by the
network mid-push records nothing, since nobody knows yet; the pair is met again.

`main` moving only by Markdown skips the merge (#587) only when the head merges
clean ([`merges_clean`]): a head that conflicts with that Markdown, say both
adding to a `sorted` list in an inert doc, goes on to the merge and the
resolver, or it would wait on a merge GitHub refuses (#1723's review). A head
that touches an agent doc is still linted merged, in a throwaway worktree with
nothing pushed ([`docs_merge_check`], #2027): two such branches can each fit
`CLAUDE.md`'s cap and overflow it together. One that fails is handed back.

A merged branch's worktree under `.claude/worktrees/` is **not** removed and
no branch is deleted — that is 12–16 GB and a checkout somebody may be
standing in. The summary names them instead.
"""

from __future__ import annotations

import os
import tempfile

from merging.queue import gitio, refusal
from merging.queue.conflict import record, resolve
from merging.verdict import changes


# The commits `main` gained since a branch's base, with every path each touched.
# `--no-renames`, so a code file renamed to `.md` shows its old path too.
MAIN_GAINED = ("log", "--no-renames", "--name-only", "--format=>%H", "origin/main")

# The last line of a conflict's hand-back: why the queue left it alone (#1723).
HANDED_BACK = (
    "Handed back unresolved: the queue resolves a conflict only inside a `sorted` list where"
    " both sides only added entries; elsewhere a machine that cannot say why the code is"
    " shaped as it is does not get to pick a side."
)

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


def diff3(work: str, named: list[str]) -> list[str]:
    """Rewrite each conflicted path in `diff3` style, one at a time, so each hunk
    shows its base to the resolver whatever the user's `merge.conflictStyle`.
    Returns the paths git could not rewrite (a modify/delete has no stage to
    rewrite from): one such path must not leave the others without a base."""
    return [path for path in named if gitio.git("checkout", "--conflict=diff3", "--", path, cwd=work).returncode != 0]


def resolved_merge(branch: str, work: str, named: list[str]) -> tuple[list[str] | None, dict[str, bool]]:
    """Finish a merge that stopped on conflicts the sorted-list rule resolves
    ([`conflict.resolve`], #1723): the notes, or `None` to hand it back, and
    whether each path resolves, for the record."""
    verdicts = resolve.settle(work, named, unreadable=diff3(work, named))
    ok = bool(named) and all(verdicts.values())
    if ok:
        staged = gitio.git("add", "--", *named, cwd=work)
        # `--no-verify`, as a clean merge commit runs no pre-commit hook either: CI judges it.
        ok = staged.returncode == 0 and gitio.git("commit", "--no-edit", "--no-verify", cwd=work).returncode == 0
    if not ok:
        return None, verdicts
    return [f"{branch}'s conflicts with `main` lay inside `sorted` lists; resolved by keeping both sides' entries, in order: {', '.join(named)}."], verdicts


def merges_clean(head: str, root: str) -> bool:
    """Whether `origin/main` merges into `head` without a conflict, asked of
    `git merge-tree` with no worktree. A git failure is *no*."""
    return gitio.git("merge-tree", "--write-tree", "--no-messages", "origin/main", head, cwd=root).returncode == 0


def handed_back(branch: str, work: str, named: list[str], stderr: str) -> list[str]:
    """Why a merge that stopped is handed back, after aborting it."""
    gitio.git("merge", "--abort", cwd=work)
    return [
        f"{branch} conflicts with `main`.",
        f"Conflicted: {', '.join(named)}." if named else f"The merge stopped; git did not name a path: {stderr.strip() or 'it said nothing'}",
        HANDED_BACK,
    ]


def skipped(head: str, why: str) -> list[str]:
    """Say that the agent-docs lint of `head`'s merged tree could not run, and
    pass it: failing open, as `lints.Linter` does for a broken tool, but never
    silently (#2027's review)."""
    print(f"queue: {head[:7]}: agent docs not linted merged ({why}); the push-to-`main` lint still holds them", flush=True)
    return []


def docs_merge_check(head: str, root: str, check) -> list[str]:
    """Why the merged tree of a head `main` passed only by Markdown must not be
    taken, or `[]`: `check` (`tools/lint`'s build-free passes, [`lints.Linter.check`])
    on `origin/main` merged into `head` in a throwaway worktree. Nothing is pushed
    and no CI round is bought: the run on record still stands when it passes. Two
    Markdown-only branches can each fit `CLAUDE.md`'s cap and overflow it merged,
    which only this sees before `main` does."""
    with tempfile.TemporaryDirectory(prefix="rusty-queue-") as tmp:
        work = os.path.join(tmp, "wt")
        if gitio.git("worktree", "add", "--detach", work, head, cwd=root).returncode != 0:
            return skipped(head, "no worktree")  # the push-to-`main` lint still holds it
        try:
            if gitio.git("merge", "--ff", "--no-edit", "--no-verify", "origin/main", cwd=work).returncode != 0:
                return skipped(head, "the merge failed")
            return check(work)
        finally:
            gitio.git("worktree", "remove", "--force", work, cwd=root)


def advance(
    branch: str, head: str, root: str, push: bool, check=None, log=None, docs_check=None
) -> tuple[str | None, list[str]]:
    """Merge `main`'s tip into `branch`. Returns the new head, or why not.

    In a detached worktree this creates and removes, never one an agent may be
    standing in. The push goes from inside it, a fast-forward leased on `head`.
    With `push=False` (the dry run) the merge is only computed. `check`, given the
    updated tree's path, returns why not to push it ([`checks.verify`]); it runs only
    when the merge moved the head. A head `main` passed only by Markdown is
    returned as it is, with [`DOCS_ONLY`] as its note ([`docs_advance`]), unless
    it conflicts with that Markdown, or `docs_check` (given when the head touches
    an agent doc) finds the merged tree broken ([`docs_merge_check`]): it is then
    handed back with that reason, unpushed. A conflict the sorted-list rule resolves is
    committed and goes on like a clean merge, with a note saying so
    ([`resolved_merge`]); `log(main, paths, ended)` records each conflict once
    its outcome is known ([`conflict.record.note`])."""
    # Only when the head merges clean: a conflict goes on to the merge and the
    # resolver, or it would wait on a merge GitHub refuses (#1723's review).
    if main_moved_by_docs(head, root) and merges_clean(head, root):
        broken = docs_merge_check(head, root, docs_check) if docs_check else []
        return (None, broken) if broken else (head, [f"{DOCS_ONLY}."])
    main = gitio.git("rev-parse", "origin/main", cwd=root).stdout.strip()
    verdicts: dict[str, bool] = {}

    def ended(fresh: str | None, notes: list[str]) -> tuple[str | None, list[str]]:
        # Recorded once the outcome is known: pushed (or, in a dry run, would be).
        if log is not None and verdicts:
            log(main, verdicts, record.RESOLVED if fresh is not None else record.HANDED_BACK)
        return fresh, notes

    with tempfile.TemporaryDirectory(prefix="rusty-queue-") as tmp:
        work = os.path.join(tmp, "wt")
        made = gitio.git("worktree", "add", "--detach", work, head, cwd=root)
        if made.returncode != 0:
            return None, [f"could not check {branch} out: {made.stderr.strip()}"]
        try:
            # `--ff` (the flag form of `-c merge.ff=true`): a user's `merge.ff=false`
            # or `only` must not turn a no-op into a merge commit or refuse the merge.
            done = gitio.git("merge", "--ff", "--no-edit", "origin/main", cwd=work)
            notes = []
            if done.returncode != 0:
                named = conflicts(gitio.git("diff", "--name-only", "--diff-filter=U", cwd=work).stdout)
                notes, verdicts = resolved_merge(branch, work, named)
                if notes is None:
                    return ended(None, handed_back(branch, work, named, done.stderr))
            fresh = gitio.git("rev-parse", "HEAD", cwd=work).stdout.strip()
            if check is not None and push_needed(head, fresh):
                broken = check(work)
                if broken:
                    return ended(None, broken)
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
                        return ended(None, refusal.explain(branch, pushed.stderr, fresh, held=held))
            return ended(fresh, notes)
        finally:
            gitio.git("worktree", "remove", "--force", work, cwd=root)
