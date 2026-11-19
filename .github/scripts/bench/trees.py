"""`main` and a pull request's head, each in a detached worktree of its own (#2381).

A PR's head is fetched as `pull/<n>/head` into a ref of its own
(`refs/bench/pr-<n>`), never as a named branch: a fork's PR has none here, a
branch name can move during a run, and `FETCH_HEAD` is whatever the last fetch
wrote. Each tree lives under the main checkout's `.claude/worktrees/` (CLAUDE.md:
one worktree per branch, never a shared `CARGO_TARGET_DIR`) and builds into its
own `target/`, named to cargo with `--target-dir`, so neither
`CARGO_BUILD_TARGET_DIR` nor a cargo config can leave a stale binary where the
run looks for it. A tree is re-pointed with a plain `checkout --detach`, so one
with local changes fails rather than losing them.

**The PR's leg measures the PR's head with the measured `origin/main` merged in**
([`merge_main`]), a local merge in the detached tree, never pushed: a PR
branched before a speed-up merged would otherwise show `main`'s speed-up as its
own regression. So the two legs differ by the PR's diff alone; a conflict
refuses, saying to update the branch. After the run the tree goes back to the
PR's head ([`checkout`]), so `make reclaim` judges it by the PR's commit and
removes it once the PR is merged or closed.

**`bench-main` is kept warm between runs**: its cold release build is most of a
run's wait, so `make reclaim` keeps the worktree named [`MAIN_TREE`] by its name
(`reclaim.finished`), and an operator who wants its disk back removes it by hand
(`git worktree remove .claude/worktrees/bench-main`).

**Both trees are locked for the run** ([`lock`]): `git worktree lock --reason
"bench-ab pid <n>"`, unlocked when the run ends however it ends but a power
loss. `make reclaim` protects a locked worktree, so a `main` tree detached at
an `origin/main` commit is not removed between legs, an hour into a run. A lock
whose PID no longer runs is clear to take over: the next run unlocks it here, and
reclaim reads the reason back ([`lock_pid_of`]) and judges the tree as unlocked.
A reused PID only errs toward keeping.
"""

from __future__ import annotations

import os
import re
import subprocess
from pathlib import Path

from merging.watch import tenancy

MAIN_TREE = "bench-main"

# How a run's worktree lock reads; the PID follows.
LOCK_REASON = "bench-ab pid"
_LOCK = re.compile(rf"^{re.escape(LOCK_REASON)} (\d+)$")
_PR_TREE = re.compile(r"^pr-(\d+)-head$")


def pr_tree(pr: int) -> str:
    """The worktree folder a PR's head is checked out in."""
    return f"pr-{pr}-head"


def pr_of_tree(name: str) -> int | None:
    """The PR a tree folder named by [`pr_tree`] is for, `None` for any other name."""
    found = _PR_TREE.match(name)
    return int(found.group(1)) if found else None


def pr_ref(pr: int) -> str:
    """The local ref a PR's head is fetched into."""
    return f"refs/bench/pr-{pr}"


def run(argv, cwd=None, env=None) -> str:
    """`argv`'s stdout; a failure raises with its stderr."""
    done = subprocess.run(argv, cwd=cwd, env=env, text=True, capture_output=True)
    if done.returncode != 0:
        raise RuntimeError(f"{' '.join(map(str, argv))}: {done.stderr.strip() or done.stdout.strip()}")
    return done.stdout


def main_checkout(git=run) -> Path:
    """The main checkout (the common git dir's parent), wherever this runs from."""
    common = Path(git(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"]).strip())
    return common.parent


def fetch(pr: int | None, git=run) -> tuple[str, str | None]:
    """The commits of `origin/main` and of PR `pr`'s head, fetched now."""
    git(["git", "fetch", "--quiet", "origin", "main"])
    main = git(["git", "rev-parse", "origin/main"]).strip()
    if pr is None:
        return main, None
    git(["git", "fetch", "--quiet", "origin", f"+pull/{pr}/head:{pr_ref(pr)}"])
    return main, git(["git", "rev-parse", pr_ref(pr)]).strip()


def path(root: Path, name: str) -> Path:
    """Where the tree `name` lives: `<root>/.claude/worktrees/<name>`."""
    return root / ".claude" / "worktrees" / name


def checkout(root: Path, name: str, sha: str, git=run) -> Path:
    """`sha` checked out detached in `<root>/.claude/worktrees/<name>`, unlocked (`pr-tree.py`)."""
    tree = path(root, name)
    if (tree / ".git").exists():
        git(["git", "-C", str(tree), "checkout", "--quiet", "--detach", sha])
    else:
        git(["git", "-C", str(root), "worktree", "add", "--quiet", "--detach", str(tree), sha])
    return tree


# Who the local merge commit is by: it is never pushed, but git wants a name.
MERGER = ["-c", "user.name=bench-ab", "-c", "user.email=bench-ab@localhost"]


def merge_main(tree: Path, pr: int, main_sha: str, git=run) -> str:
    """`main_sha` merged into the PR's head checked out in `tree`, the merged commit.

    A conflict aborts the merge and refuses: the branch is updated, not the bench."""
    try:
        git(["git", "-C", str(tree), *MERGER, "merge", "--quiet", "--no-edit", main_sha])
    except RuntimeError as e:
        try:
            git(["git", "-C", str(tree), "merge", "--abort"])
        except RuntimeError:
            pass
        raise RuntimeError(f"PR #{pr} does not merge cleanly with main {main_sha[:9]}: update the branch "
                           f"from main, then bench it ({e})") from None
    return git(["git", "-C", str(tree), "rev-parse", "HEAD"]).strip()


def lock_pid_of(reason: str | None) -> int | None:
    """The run's PID a worktree lock's reason names, or `None` for any other lock."""
    found = _LOCK.match((reason or "").strip())
    return int(found.group(1)) if found else None


def locked_reason(root: Path, tree: Path, git=run) -> str | None:
    """The lock reason on `tree` (`""` for none given), `None` when it is not locked."""
    listing = git(["git", "-C", str(root), "worktree", "list", "--porcelain"])
    for block in listing.strip().split("\n\n"):
        lines = block.splitlines()
        if lines and lines[0] == f"worktree {tree}":
            for line in lines:
                if line == "locked" or line.startswith("locked "):
                    return line.partition(" ")[2]
            return None
    return None


def lock(root: Path, tree: Path, pid: int, git=run, alive=tenancy.alive, exclusive: bool = False) -> None:
    """Lock `tree` for run `pid`, taking over a lock a dead run left; any other lock raises.

    `exclusive`: this run holds the timed-run `flock`, so every `bench-ab pid`
    lock it finds is stale, whatever `alive` says of a reused PID."""
    held = locked_reason(root, tree, git)
    if held is not None:
        owner = lock_pid_of(held)
        if owner is None:
            raise RuntimeError(f"{tree} is locked ({held or 'no reason given'}): not ours to take")
        if owner != pid and alive(owner) and not exclusive:
            raise RuntimeError(f"{tree} is locked ({held}), and PID {owner} runs: not ours to take. If no "
                               f"`make bench-ab` runs (a power cut left the lock, its PID reused), "
                               f"`git worktree unlock {tree}` and run again")
        git(["git", "-C", str(root), "worktree", "unlock", str(tree)])
    git(["git", "-C", str(root), "worktree", "lock", "--reason", f"{LOCK_REASON} {pid}", str(tree)])


def take(root: Path, name: str, sha: str, pid: int, locked: list, git=run, alive=tenancy.alive,
         exclusive: bool = False) -> Path:
    """`sha` checked out detached in the tree `name`, locked for run `pid`; the tree is
    appended to `locked` the moment it is locked, so the caller unlocks only its own.

    An existing tree is locked first and re-pointed after: a live run's lock
    refuses before its sources move."""
    tree = path(root, name)
    if (tree / ".git").exists():
        lock(root, tree, pid, git, alive, exclusive)
        locked.append(tree)
        git(["git", "-C", str(tree), "checkout", "--quiet", "--detach", sha])
    else:
        git(["git", "-C", str(root), "worktree", "add", "--quiet", "--detach", str(tree), sha])
        lock(root, tree, pid, git, alive, exclusive)
        locked.append(tree)
    return tree


def held_by_run(root: Path, tree: Path, git=run, alive=tenancy.alive) -> int | None:
    """The PID of the live `make bench-ab` run that locks `tree`, if one does."""
    if not (tree / ".git").exists():
        return None
    owner = lock_pid_of(locked_reason(root, tree, git))
    return owner if owner is not None and alive(owner) else None


def unlock(root: Path, tree: Path, pid: int, git=run) -> None:
    """Drop run `pid`'s lock on `tree`, if it still holds it; never raises."""
    try:
        if lock_pid_of(locked_reason(root, tree, git)) == pid:
            git(["git", "-C", str(root), "worktree", "unlock", str(tree)])
    except RuntimeError:
        pass


def build_env(env=None) -> dict:
    """The environment a tree builds in: its own `target/`, never a shared one."""
    env = dict(os.environ if env is None else env)
    env.pop("CARGO_TARGET_DIR", None)
    env.pop("CARGO_BUILD_TARGET_DIR", None)
    return env


BUILD = ["cargo", "build", "--release", "--locked", "--quiet", "--features", "dev", "--bin", "bench-scenes"]


def build(tree: Path, git=run) -> Path:
    """Build `tree`'s `bench-scenes` (release, `dev`) into `<tree>/target`, returning the binary."""
    target = tree / "target"
    git([*BUILD, "--target-dir", str(target)], cwd=tree, env=build_env())
    return target / "release" / "bench-scenes"
