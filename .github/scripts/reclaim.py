#!/usr/bin/env python3
"""`reclaim.py [--dry-run]` — free the disk finished branches left behind (#913).

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
- a detached HEAD is finished when it is an ancestor of `origin/main`.

A finished worktree is still **never** removed when it has uncommitted
changes, commits on no remote (and not in its PR's head), a lock (a live
agent session holds it), a running process standing in it, or is the one this
runs from; those are named instead. The process check exists because the merge
queue runs from a detached worktree here (`queue/`, its warm build in
`target/merge-queue`): a detached HEAD on `origin/main` looks finished, and on
the first dry run one was, while `make queue` stood in it. Kept worktrees print their `target/` size. Then `git worktree prune`,
and free disk before and after. Anything under `.claude/worktrees/` that is
not a git worktree (`.cargo/`, tarballs) is left alone.

`--dry-run` decides and prints the same, and deletes nothing.
"""

from __future__ import annotations

import contextlib
import json
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

USAGE = "usage: reclaim.py [--dry-run]"


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
    if wt.branch is None:
        if facts.on_main:
            return True, "detached at a commit on origin/main"
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
    if wt.locked is not None:
        return "protect", f"{why}, but locked ({wt.locked or 'no reason given'})"
    if facts.dirty:
        return "protect", f"{why}, but it has uncommitted changes"
    if facts.unpushed:
        return "protect", f"{why}, but {facts.unpushed} commit(s) are on no remote"
    return "remove", why


def inside(path: Path, tree: Path) -> bool:
    return path == tree or tree in path.parents


def gib(n_bytes: int) -> str:
    return f"{n_bytes / 2**30:.1f} GiB"


class Machine:
    """Every effect reclaim has; tests inject a fake `runner` and replace the rest."""

    def __init__(self, runner=subprocess.run):
        self.runner = runner

    def run(self, *argv: str, cwd: Path | None = None) -> subprocess.CompletedProcess:
        return self.runner(list(argv), capture_output=True, text=True, check=False, cwd=cwd)

    def git(self, cwd: Path, *args: str) -> str:
        done = self.run("git", "-C", str(cwd), *args)
        if done.returncode != 0:
            sys.exit(f"reclaim: git {' '.join(args)}: {done.stderr.strip()}")
        return done.stdout.strip()

    def is_ancestor(self, cwd: Path, commit: str, of: str) -> bool:
        return self.run("git", "-C", str(cwd), "merge-base", "--is-ancestor", commit, of).returncode == 0

    def pulls(self, branch: str) -> list[dict] | None:
        done = self.run("gh", "pr", "list", "--state", "all", "--head", branch,
                        "--json", "state,headRefOid")
        return json.loads(done.stdout or "[]") if done.returncode == 0 else None

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
        facts = Facts(current=inside(here, wt.path))
        facts.busy = None if cwds is None else [pid for pid, cwd in cwds if inside(cwd, wt.path)]
        facts.dirty = bool(self.git(wt.path, "status", "--porcelain"))
        if wt.branch is None:
            facts.on_main = self.is_ancestor(wt.path, wt.head, "origin/main")
            return facts
        pulls = self.pulls(wt.branch)
        facts.prs = None if pulls is None else [p["state"] for p in pulls]
        stray = int(self.git(wt.path, "rev-list", "--count", "HEAD", "--not", "--remotes"))
        # A merged PR's branch is often deleted on the remote, which strands its
        # commits locally; they are safe when the PR's head already holds them.
        heads = [p["headRefOid"] for p in pulls or [] if p.get("headRefOid")]
        if stray and not any(self.is_ancestor(wt.path, wt.head, h) for h in heads):
            facts.unpushed = stray
        return facts

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

    worktrees = [wt for wt in parse_worktrees(machine.git(main_checkout, "worktree", "list", "--porcelain"))
                 if root in wt.path.parents]
    cwds = machine.cwds()
    removed = 0
    for wt in worktrees:
        action, why = verdict(wt, machine.facts(wt, here, cwds))
        name = f"{wt.path.name} ({wt.branch or 'detached ' + wt.head[:8]})"
        if action == "remove":
            if not dry:
                machine.remove(main_checkout, wt.path)
            removed += 1
            print(f"{'would remove' if dry else 'removed'}: {name}: {why}")
        else:
            size = machine.target_size(wt.path)
            shown = f"target/ {gib(size)}" if size is not None else "no target/"
            label = "PROTECTED" if action == "protect" else "kept"
            print(f"{label}: {name}: {why}; {shown}")

    if not dry:
        machine.git(main_checkout, "worktree", "prune")
    after = machine.free(main_checkout)
    verb = "would remove" if dry else "removed"
    print(f"reclaim: {verb} {removed} of {len(worktrees)} worktree(s); "
          f"free disk {gib(before)} -> {gib(after)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
