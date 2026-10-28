#!/usr/bin/env python3
"""`reclaim.py [--dry-run]` — free the disk and branches finished work left behind (#913).

Every branch gets its own worktree under `.claude/worktrees/`, and each one
compiles into its own `target/` (several GB). `issue-batch` says to remove a
worktree the moment its branch merges, but done by hand, per branch, it gets
skipped under load: on 2026-10-05 the disk hit 93% and a GPU-only branch had to
move to the cloud halfway through because its worktree held 8 GB.

This walks the git worktrees of the **main checkout** under
`.claude/worktrees/` and removes each finished one, `target/` first:

- a branch is finished when its pull request is MERGED or CLOSED (squash
  merges leave nothing in `origin/main`'s ancestry, so GitHub decides, not
  git); an OPEN one, or none at all, keeps it;
- a detached HEAD is finished when it is an ancestor of `origin/main`, or
  when it is a commit of a MERGED or CLOSED pull request and of no OPEN one
  (below).

A finished worktree is still **never** removed when it has uncommitted
changes, commits on no remote (and not in its PR's head), a lock (a live
agent session holds it), a running process standing in it, or is the one this
runs from; those are named instead. The process check exists because the merge
queue runs from a detached worktree here (`queue/`, its warm build in
`target/merge-queue`): a detached HEAD on `origin/main` looks finished, and on
the first dry run one was, while `make queue` stood in it. Kept worktrees print their `target/` size. Then `git worktree prune`,
and free disk before and after. Anything under `.claude/worktrees/` that is
not a git worktree (`.cargo/`, tarballs) is left alone.

A linked worktree **outside** `.claude/worktrees/` breaks the one-checkout-per-
branch rule and is never reclaimed, while its `target/` keeps its GBs (#982).
It is never touched either, only named: `OUTSIDE: <path> (<branch>, <target/>)`,
so the disk nobody would get back at least shows up.

The merge-queue supervisor locks its worktree for as long as it lives (#1450):
on 2026-10-25 reclaim removed `queue/` while the supervisor slept between
watches, nothing standing in it, and the supervisor died on its next `git`
call. A lock whose reason names a supervisor PID that is no longer running is
still protected, but named as stale with that PID, so a dead supervisor's lock
shows up instead of keeping the worktree forever without a word.

The merge queue rebases a branch before it merges it, so a local branch's
commits are never an ancestor of the PR's recorded head, and the remote branch
is deleted on merge (#1453: PR #1449's `session-start-fresh-main` and PR
#1452's `rebrief-one-session` both stayed PROTECTED as "on no remote" until
removed by hand). For a merged PR the unpushed commits are therefore excused
when their content is already in `origin/main`: `git merge-tree --write-tree
origin/main HEAD` yields `origin/main`'s own tree. That needs git >= 2.38;
older, reclaim keeps the stricter ancestor rule and says so.

A removed worktree's branch is deleted with it, and every local branch no
worktree holds is judged the same way, its pull request found by name or by
tip commit (#1482: 39 dead branches after the Post Processing batch, most of
them reviewers' `prNNNN`; `reclaim_branches.py` has the story). `main` and a
branch any worktree has checked out are never deleted.

`independent-review` dispatches each reviewer with the `Agent` tool's worktree
isolation, and the harness names that worktree's branch itself:
`worktree-agent-<id>`, at the commit it started from (#1510). The reviewer then
checks the PR out detached, so after a squash merge reclaim kept both forever:
the branch as "no pull request", the worktree as "detached, not on
origin/main" (PRs #1503 and #1504, 2026-10-28, each left one of each). So a
`worktree-agent-*` branch no worktree holds and whose tip is on `origin/main`
is deleted, having nothing of its own; with commits of its own it is judged
like any branch. Only that prefix: any other branch at an `origin/main` commit
is a fresh one and stays. A detached worktree whose HEAD is any commit of a
MERGED or CLOSED pull request (asked by commit, as `judge_branch` does) is
finished too; an OPEN one holding that commit keeps it. Any commit, not only
the head: a reviewer's checkout sits at the commit it reviewed, and when the
review sends fixes back the PR's head moves on (PR #1503: reviewed at
`daa7e46`, merged from `789c9d9`). The lock and the
process check still protect a running reviewer: the harness locks its
worktree (`claude agent agent-<id> (pid …)`).

That PR-by-commit rule missed the common case (#1538): GitHub lists the pull
requests of a commit not on the default branch only while they are OPEN, and
the queue rebases every branch before merging it, so a reviewed head never
reaches `main`. On 2026-10-28, after the batch merged, `make reclaim` kept 14
`agent-*` worktrees as "detached, not on origin/main" and the reviewers'
`pr1514`/`pr1531` branches as "no pull request" (PR #1509's reviewed head
`7f254256` answers `[]`). So a reviewer's checkout is judged by what it
holds, not which PR it came from: a worktree under `.claude/worktrees/agent-*`,
and a `pr<digits>…` or `worktree-agent-*` branch, whose HEAD (or tip) is a
commit GitHub has (`repos/{repo}/commits/<sha>` answers 200) holds nothing
of its own and is finished, its commits counted as pushed. The lock, a
process, a dirty tree (untracked files included) and the worktree reclaim
runs from still protect it, an OPEN pull request holding the commit still
keeps it, and GitHub not answering keeps it and says so. Every other name
keeps the rules above.

`--dry-run` decides and prints the same, and deletes nothing.
"""

from __future__ import annotations

import contextlib
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

import ghrest
import reclaim_branches as branches
from merging.watch import tenancy

USAGE = "usage: reclaim.py [--dry-run]"
AGENT_BRANCH = "worktree-agent-"  # the harness's name for an `Agent` worktree's branch (#1510)
REVIEWER_BRANCH = re.compile(rf"pr\d|{AGENT_BRANCH}")  # a reviewer's `pr1514…`, the harness's own (#1538)


@dataclass
class Worktree:
    path: Path
    head: str
    branch: str | None = None  # None when detached
    locked: str | None = None  # the lock's reason ("" when given none)


@dataclass
class Facts:
    """What decides a worktree's fate, gathered by `Machine.facts`."""

    prs: list[str] | None = field(default_factory=list)  # PR states; None: GitHub didn't answer
    on_main: bool = False  # HEAD is an ancestor of origin/main
    dirty: bool = False
    unpushed: int = 0
    current: bool = False
    busy: list[int] | None = field(default_factory=list)  # pids standing in it; None: unknown
    stale: int | None = None  # a lock left by a merge-queue supervisor that no longer runs: its pid
    reviewer: bool = False  # a reviewer's checkout: judged by what it holds (#1538)
    on_github: bool | None = False  # GitHub has its HEAD commit; None: GitHub didn't answer


def parse_worktrees(porcelain: str) -> list[Worktree]:
    """`git worktree list --porcelain` as records; blank lines separate them."""
    found: list[Worktree] = []
    for block in porcelain.strip().split("\n\n"):
        fields = dict(line.partition(" ")[::2] for line in block.splitlines() if line)
        if "worktree" not in fields:
            continue
        branch = fields.get("branch")
        found.append(Worktree(
            path=Path(fields["worktree"]),
            head=fields.get("HEAD", ""),
            branch=branch.removeprefix("refs/heads/") if branch else None,
            locked=fields.get("locked"),
        ))
    return found


def finished(wt: Worktree, facts: Facts) -> tuple[bool, str]:
    """Is this worktree's work over, and why (or why not)."""
    done, why = by_pull_request(wt, facts)
    if done or not facts.reviewer or facts.prs is None or "OPEN" in facts.prs:
        return done, why
    if facts.on_github is None:
        return False, f"{why}, and GitHub did not answer whether it has the commit"
    if facts.on_github:
        return True, "a reviewer's checkout at a commit GitHub has"
    return False, why


def by_pull_request(wt: Worktree, facts: Facts) -> tuple[bool, str]:
    """`finished` by the pull requests and `origin/main` alone (#913, #1510)."""
    if wt.branch is None:
        if facts.on_main:
            return True, "detached at a commit on origin/main"
        if facts.prs is None:
            return False, "detached, not on origin/main, and GitHub did not answer for its pull request"
        if "OPEN" in facts.prs:
            return False, "detached at a commit of an open pull request"
        for state in ("MERGED", "CLOSED"):
            if state in facts.prs:
                return True, f"detached at a commit of a {state.lower()} pull request"
        return False, "detached, not on origin/main"
    if facts.prs is None:
        return False, "GitHub did not answer for its pull request"
    if "OPEN" in facts.prs:
        return False, "pull request open"
    if "MERGED" in facts.prs:
        return True, "pull request merged"
    if "CLOSED" in facts.prs:
        return True, "pull request closed"
    return False, "no pull request"


def verdict(wt: Worktree, facts: Facts) -> tuple[str, str]:
    """("remove" | "protect" | "keep", why). Protect: finished, but unsafe to delete."""
    done, why = finished(wt, facts)
    if not done:
        return "keep", why
    if facts.current:
        return "protect", f"{why}, but this is the worktree reclaim runs from"
    if facts.busy is None:
        return "protect", f"{why}, but no way to tell whether a process stands in it"
    if facts.busy:
        pids = ", ".join(map(str, facts.busy))
        return "protect", f"{why}, but a running process stands in it (pid {pids})"
    if wt.locked is not None and facts.stale is not None:
        return "protect", (f"{why}, but locked by merge-queue supervisor pid {facts.stale}, which is not running:"
                           f" a stale lock; `git worktree unlock {wt.path}` to let reclaim remove it")
    if wt.locked is not None:
        return "protect", f"{why}, but locked ({wt.locked or 'no reason given'})"
    if facts.dirty:
        return "protect", f"{why}, but it has uncommitted changes"
    if facts.unpushed:
        return "protect", f"{why}, but {facts.unpushed} commit(s) are on no remote"
    return "remove", why


def judge_branch(machine: Machine, main: Path, branch: str, tip: str) -> tuple[str, str]:
    """A branch no worktree holds, judged like a worktree's (#1482).

    Its pull request is found by name, else by tip commit, which is how a
    reviewer's `pr1316` finds PR #1316. A tip already on `origin/main` is not
    asked by commit: GitHub would answer with whichever PR merged that commit,
    and a fresh branch off `main` would read as finished. The one exception is
    the harness's `worktree-agent-*` branch (#1510): on `origin/main` it holds
    nothing of its own.
    """
    on_main = machine.is_ancestor(main, tip, "origin/main")
    if branch.startswith(AGENT_BRANCH) and on_main:
        return "remove", "an agent worktree's branch; nothing of its own"
    pulls = machine.pulls(branch)
    if pulls == [] and not on_main:
        pulls = machine.pulls_of(tip)
    facts = Facts(reviewer=bool(REVIEWER_BRANCH.match(branch)))
    return verdict(Worktree(main, tip, branch), machine.work(facts, main, branch, tip, pulls))


def reviewer_tree(wt: Worktree) -> bool:
    """The `Agent` tool's isolation: `.claude/worktrees/agent-<id>` (#1538)."""
    return wt.path.name.startswith("agent-") and wt.path.parent.parts[-2:] == (".claude", "worktrees")


def inside(path: Path, tree: Path) -> bool:
    return path == tree or tree in path.parents


def gib(n_bytes: int) -> str:
    return f"{n_bytes / 2**30:.1f} GiB"


def describe(wt: Worktree) -> str:
    return wt.branch or "detached " + wt.head[:8]


class Machine:
    """Every effect reclaim has; tests inject a fake `runner` and replace the rest."""

    def __init__(self, runner=subprocess.run):
        self.runner = runner
        self.old_git = False  # git < 2.38 has no `merge-tree --write-tree`: noted once in main

    def run(self, *argv: str, cwd: Path | None = None) -> subprocess.CompletedProcess:
        return self.runner(list(argv), capture_output=True, text=True, check=False, cwd=cwd)

    def git(self, cwd: Path, *args: str) -> str:
        done = self.run("git", "-C", str(cwd), *args)
        if done.returncode != 0:
            sys.exit(f"reclaim: git {' '.join(args)}: {done.stderr.strip()}")
        return done.stdout.strip()

    def is_ancestor(self, cwd: Path, commit: str, of: str) -> bool:
        return self.run("git", "-C", str(cwd), "merge-base", "--is-ancestor", commit, of).returncode == 0

    def merged_into_main(self, cwd: Path, rev: str = "HEAD") -> bool:
        """Is `rev`'s content already in `origin/main` (a rebase or squash can't hide it)?"""
        words = self.run("git", "--version").stdout.split()
        version = words[-1].split(".") if words else []
        if tuple(int(n) if n.isdigit() else 0 for n in version[:2]) < (2, 38):
            self.old_git = True
            return False
        merged = self.run("git", "-C", str(cwd), "merge-tree", "--write-tree", "origin/main", rev)
        if merged.returncode != 0:  # a conflict: HEAD holds something main doesn't
            return False
        main_tree = self.run("git", "-C", str(cwd), "rev-parse", "origin/main^{tree}")
        return bool(merged.stdout.strip()) and merged.stdout.split()[0] == main_tree.stdout.strip()

    def pulls(self, branch: str) -> list[dict] | None:
        """`branch`'s pull requests in any state, over REST (#970); `None` if unknown."""
        repo = ghrest.slug_of(self.run("git", "remote", "get-url", "origin").stdout)
        if repo is None:
            return None
        return self.rest(f"repos/{repo}/pulls?state=all&head={repo.split('/')[0]}:{branch}&per_page=100")

    def pulls_of(self, commit: str) -> list[dict] | None:
        """The pull requests holding `commit` (#1482: a reviewer's `pr1316` is no PR's head ref)."""
        repo = ghrest.slug_of(self.run("git", "remote", "get-url", "origin").stdout)
        return None if repo is None else self.rest(f"repos/{repo}/commits/{commit}/pulls?per_page=100")

    def has_commit(self, commit: str) -> bool | None:
        """Does GitHub have `commit`? It answers 422 (or 404) for one it doesn't; `None` if unknown."""
        repo = ghrest.slug_of(self.run("git", "remote", "get-url", "origin").stdout)
        if repo is None:
            return None
        done = self.run("gh", "api", f"repos/{repo}/commits/{commit}", "--jq", ".sha")
        if done.returncode == 0:
            return True
        return False if any(f"HTTP {code}" in done.stderr for code in (404, 422)) else None

    def rest(self, path: str) -> list[dict] | None:
        done = self.run("gh", "api", path)
        return [ghrest.pull(p) for p in json.loads(done.stdout or "[]")] if done.returncode == 0 else None

    def cwds(self) -> list[tuple[int, Path]] | None:
        """Every process's working directory: /proc on Linux, lsof elsewhere."""
        proc = Path("/proc")
        if (proc / "self" / "cwd").exists():
            found = []
            for link in proc.glob("[0-9]*/cwd"):
                with contextlib.suppress(OSError):
                    found.append((int(link.parent.name), Path(os.readlink(link))))
            return found
        try:
            done = self.run("lsof", "-d", "cwd", "-Fpn")
        except OSError:
            return None
        if not done.stdout:
            return None
        found, pid = [], 0
        for line in done.stdout.splitlines():
            if line.startswith("p"):
                pid = int(line[1:])
            elif line.startswith("n"):
                found.append((pid, Path(line[1:])))
        return found

    def facts(self, wt: Worktree, here: Path, cwds: list[tuple[int, Path]] | None) -> Facts:
        facts = Facts(current=inside(here, wt.path), reviewer=reviewer_tree(wt))
        facts.busy = None if cwds is None else [pid for pid, cwd in cwds if inside(cwd, wt.path)]
        facts.dirty = bool(self.git(wt.path, "status", "--porcelain"))
        owner = tenancy.pid_of(wt.locked)
        if owner is not None and not self.alive(owner):
            facts.stale = owner
        if wt.branch is None:
            facts.on_main = self.is_ancestor(wt.path, wt.head, "origin/main")
            if not facts.on_main:  # a reviewer's checkout of a PR (#1510): any PR holding the commit counts
                pulls = self.pulls_of(wt.head)
                facts.prs = None if pulls is None else [p["state"] for p in pulls]
                if facts.reviewer:
                    facts.on_github = self.has_commit(wt.head)
            return facts
        return self.work(facts, wt.path, wt.branch, wt.head, self.pulls(wt.branch))

    def work(self, facts: Facts, cwd: Path, branch: str, head: str, pulls: list[dict] | None) -> Facts:
        """`branch`'s pull request states, and its commits that nothing but this checkout holds."""
        facts.prs = None if pulls is None else [p["state"] for p in pulls]
        if facts.reviewer:  # its commits are on GitHub, if not in any ref fetched here (#1538)
            facts.on_github = self.has_commit(head)
            if facts.on_github:
                return facts
        stray = int(self.git(cwd, "rev-list", "--count", head, "--not", "--remotes"))
        # A merged PR's branch is often deleted on the remote, which strands its
        # commits locally; they are safe when the PR's head already holds them.
        heads = [p["headRefOid"] for p in pulls or [] if p.get("headRefOid")]
        if stray and not any(self.is_ancestor(cwd, head, h) for h in heads):
            if "MERGED" not in (facts.prs or []) or not self.merged_into_main(cwd, head):
                facts.unpushed = stray
        return facts

    def alive(self, pid: int) -> bool:
        return tenancy.alive(pid)

    def target_size(self, wt: Path) -> int | None:
        target = wt / "target"
        if not target.is_dir():
            return None
        done = self.run("du", "-sk", str(target))
        return int(done.stdout.split()[0]) * 1024 if done.returncode == 0 else None

    def free(self, path: Path) -> int:
        return shutil.disk_usage(path).free

    def remove(self, main: Path, wt: Path) -> None:
        shutil.rmtree(wt / "target", ignore_errors=True)
        self.git(main, "worktree", "remove", str(wt))

    def delete_branch(self, main: Path, branch: str) -> str | None:
        """`git branch -D`; the error when git refused, so one branch can't stop the run."""
        done = self.run("git", "-C", str(main), "branch", "-D", branch)
        return done.stderr.strip() or "failed" if done.returncode != 0 else None


def shown_target(machine: Machine, wt: Path) -> str:
    size = machine.target_size(wt)
    return f"target/ {gib(size)}" if size is not None else "no target/"


def main(argv: list[str], machine: Machine | None = None, here: Path | None = None) -> int:
    machine = machine or Machine()
    unknown = [a for a in argv if a != "--dry-run"]
    if unknown:
        sys.exit(f"reclaim: unknown argument(s) {' '.join(unknown)}; {USAGE}")
    dry = "--dry-run" in argv
    here = (here or Path.cwd()).resolve()
    common = Path(machine.git(here, "rev-parse", "--path-format=absolute", "--git-common-dir"))
    main_checkout = common.parent
    root = main_checkout / ".claude" / "worktrees"
    before = machine.free(main_checkout)
    machine.run("git", "-C", str(main_checkout), "fetch", "--quiet", "origin")

    listed = parse_worktrees(machine.git(main_checkout, "worktree", "list", "--porcelain"))
    worktrees = [wt for wt in listed if root in wt.path.parents]
    outside = [wt for wt in listed if root not in wt.path.parents and wt.path != main_checkout]
    cwds = machine.cwds()
    removed, deleted = 0, 0
    for wt in worktrees:
        action, why = verdict(wt, machine.facts(wt, here, cwds))
        name = f"{wt.path.name} ({describe(wt)})"
        if action == "remove":
            if not dry:
                machine.remove(main_checkout, wt.path)
            removed += 1
            print(f"{'would remove' if dry else 'removed'}: {name}: {why}")
            if wt.branch is not None and wt.branch != "main":
                deleted += branches.delete(machine, main_checkout, wt.branch, "its worktree was removed", dry)
        else:
            label = "PROTECTED" if action == "protect" else "kept"
            print(f"{label}: {name}: {why}; {shown_target(machine, wt.path)}")

    held = {wt.branch for wt in listed if wt.branch is not None}
    deleted += branches.visit(machine, main_checkout, held, dry, judge_branch)

    for wt in outside:
        print(f"OUTSIDE: {wt.path} ({describe(wt)}, {shown_target(machine, wt.path)}): "
              f"not under {root}, so never reclaimed")

    if not dry:
        machine.git(main_checkout, "worktree", "prune")
    after = machine.free(main_checkout)
    if machine.old_git:
        print("reclaim: note: git < 2.38 cannot check content against origin/main; "
              "merged branches rebased by the queue stay PROTECTED")
    verb = "would remove" if dry else "removed"
    print(f"reclaim: {verb} {removed} of {len(worktrees)} worktree(s) and {deleted} branch(es); "
          f"free disk {gib(before)} -> {gib(after)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
