"""The local branches `reclaim.py` deletes, beside the worktrees it removes (#1482).

At the end of the Post Processing batch (#1446, 2026-10-26) the main checkout
held 39 local branches after `make reclaim` had removed every merged worktree.
Two sources left them:

- reclaim removed a finished worktree but never its branch
  (`1433-camera-motion-blur-controls`, `rebase-not-merge`, …);
- `independent-review` reviewers fetched each PR into a named branch, every one
  spelled differently (`pr1316`, `pr-1316`, `pr1314-review`, `pr1403rev`), and
  their worktrees went while the branches stayed.

Sweeping them with `git branch -D` by hand was refused as irreversible and the
operator had to authorise it in chat; meanwhile the pile hid the one branch
with real unmerged work. So a removed worktree's branch goes with it, under the
proof that removed the worktree, and every other local branch no worktree holds
is judged by the same rules (`reclaim.judge_branch`): finished when its pull
request, found by name or by tip commit, is MERGED or CLOSED; never deleted
with commits on no remote that its PR's head and `origin/main` don't hold.
`main` and any branch a worktree has checked out are never touched.

A reviewer's `worktree-agent-<id>`, the branch the harness gives an `Agent`
worktree, is deleted when its tip is on `origin/main` (#1510,
`reclaim.judge_branch`): the reviewer checked its PR out detached, so the
branch holds nothing. No other name gets that rule.
"""

from __future__ import annotations

from pathlib import Path


def local(machine, main: Path) -> list[tuple[str, str]]:
    """Every `refs/heads/` branch as (name, tip)."""
    listed = machine.git(main, "for-each-ref", "--format=%(refname:short) %(objectname)", "refs/heads")
    return [tuple(line.split(" ", 1)) for line in listed.splitlines() if line]


def delete(machine, main: Path, branch: str, why: str, dry: bool) -> int:
    """Delete `branch` (or say it would); 1 when it went, for the summary's count."""
    if dry:
        print(f"would delete branch: {branch}: {why}")
        return 1
    error = machine.delete_branch(main, branch)
    if error:
        print(f"PROTECTED branch: {branch}: {why}, but git refused to delete it: {error}")
        return 0
    print(f"deleted branch: {branch}: {why}")
    return 1


def visit(machine, main: Path, held: set[str], dry: bool, judge) -> int:
    """Judge each branch no worktree holds; delete the finished, name the rest."""
    deleted = 0
    for branch, tip in local(machine, main):
        if branch == "main" or branch in held:
            continue
        action, why = judge(machine, main, branch, tip)
        if action == "remove":
            deleted += delete(machine, main, branch, why, dry)
        else:
            print(f"{'PROTECTED' if action == 'protect' else 'kept'} branch: {branch}: {why}")
    return deleted
