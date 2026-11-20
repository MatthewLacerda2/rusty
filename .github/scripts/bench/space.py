"""Free disk before a heavy build, and what to reclaim when there is not enough (#2436).

On the operator's desktop, during a day of measurements, `make test-gpu
PR=2419` built its debug test profile into its tree's own `target/`, 11 GiB for
one PR (a bench tree's release build is about 1.7 GiB), and the next `make
test-gpu PR=2426` ran the disk to 0 bytes free mid-build. From then on every git
command that writes failed, `make reclaim` among them, and what shares the disk
(the merge-queue supervisor's worktree and log, sessions, other projects' jobs)
was at risk. So the heavy verbs read the free space of the filesystem holding
`.claude/worktrees/` and refuse **before** building when it is under what the
build still needs ([`refusal`]): the build's size ([`NEEDS`]) less what the
tree's own `target/` already holds of that profile, plus each second binary the
same invocation links ([`EXTRA`]: `make bench-ab STRESS=1`'s `play`) that the
profile's folder does not hold yet, plus [`HEADROOM`] kept free for everything
else. The refusal names the free figure, the need, `make reclaim`
and the trees holding the most, largest first. The figures are measured, never a
machine's free space: the check reads the machine each run.

A tool that hits a full disk anyway says so ([`full`], [`said_full`]) with the
same advice, rather than git's bare `index.lock write error`.
"""

from __future__ import annotations

import shutil
from pathlib import Path

GIB = 2**30
# What a cold build writes into a tree's `target/<profile>`, measured: a bench
# tree's release `bench-scenes` is about 1.7 GiB (the operator's desktop); a
# `make test-gpu PR=` build of the `dev` test profile 11 GiB there (PR #2419) and
# 8.7 GiB in a 4-core Linux cloud container (#2436: 6 min 35 s cold, which is also what
# a re-run pays after a green run deletes it). Rounded up, so a build that grows
# a little still fits.
NEEDS = {"release": 2 * GIB, "debug": 12 * GIB}
# A second binary linked in the same cargo invocation, beyond what `NEEDS` holds:
# its own objects and the binary, the crates being shared: `play` beside a cold
# release `bench-scenes` grew `target/release` from 1.58 to 1.63 GiB, 57 MiB, in a
# 4-core Linux cloud container (#2450; 7 min 12 s for `bench-scenes`, 3 min 31 s more
# for `play`). Rounded up, as `NEEDS` is.
EXTRA = {"play": GIB // 4}
HEADROOM = 2 * GIB  # left free after the build: git, logs, the queue supervisor
SHOWN = 5  # trees named in a refusal, largest first
# What git and the OS say when a write found the disk full (Linux; git's own; macOS).
FULL = ("no space left on device", "out of diskspace", "disk full")


def gib(n_bytes: int) -> str:
    return f"{n_bytes / GIB:.1f} GiB"


def du(path: Path, run) -> int:
    """Bytes under `path` (`du -sk`), 0 when it is absent or `du` fails; `run` is `subprocess.run`."""
    if not path.exists():
        return 0
    done = run(["du", "-sk", str(path)], capture_output=True, text=True, check=False)
    words = done.stdout.split()
    return int(words[0]) * 1024 if done.returncode == 0 and words and words[0].isdigit() else 0


def largest(root: Path, run) -> list[tuple[str, int]]:
    """`(tree, bytes of its target/)` for every tree under `<root>/.claude/worktrees/`, largest first."""
    base = root / ".claude" / "worktrees"
    found = [(d.name, du(d / "target", run)) for d in sorted(base.iterdir()) if d.is_dir()] if base.is_dir() else []
    return sorted((f for f in found if f[1] > 0), key=lambda f: -f[1])


def advice(root: Path, run) -> str:
    """What frees the disk: `make reclaim`, and the trees whose `target/` holds the most."""
    held = largest(root, run)[:SHOWN]
    named = ", ".join(f"{name} {gib(size)}" for name, size in held) or "none holds a target/"
    return (f"`make reclaim` removes finished trees, and `rm -rf <tree>/target` frees a kept one no build "
            f"runs in; the largest under .claude/worktrees/: {named}")


def still(tree: Path, profile: str, extra: tuple[str, ...], run) -> int:
    """Bytes a build of `profile` into `<tree>/target` still writes: [`NEEDS`] less what
    the profile's folder holds, plus each `extra` binary ([`EXTRA`]) not yet built there."""
    built = tree / "target" / profile
    more = max(NEEDS[profile] - du(built, run), 0)
    return more + sum(EXTRA[name] for name in extra if not (built / name).exists())


def refusal(root: Path, builds: list[tuple], what: str, run, free=None) -> str | None:
    """Why `what` must not start, or `None` when it fits: `builds` are `(tree, profile)`,
    each a build of `profile` into `<tree>/target`, or `(tree, profile, extra)` with
    the second binaries ([`EXTRA`]) the same invocation links.

    `free(path)` is the free bytes of the filesystem holding `path`
    (`shutil.disk_usage`, faked in tests); what a tree's `target/<profile>`
    already holds is counted as built."""
    free = free or (lambda p: shutil.disk_usage(p).free)
    more = [(b[0], b[1], still(b[0], b[1], tuple(b[2]) if len(b) > 2 else (), run)) for b in builds]
    need = sum(m for _, _, m in more) + HEADROOM
    base = root / ".claude" / "worktrees"
    left = free(base if base.is_dir() else root)
    if left >= need:
        return None
    parts = ", ".join(f"{tree.name}/target/{profile} {gib(m)}" for tree, profile, m in more)
    return (f"{gib(left)} free on the disk holding {base}, and {what} needs {gib(need)} "
            f"(build output still to write: {parts}; {gib(HEADROOM)} kept free): {advice(root, run)}")


def said_full(text: str) -> bool:
    """Does an error's text say the disk is full?"""
    low = text.lower()
    return any(word in low for word in FULL)


def full(text: str, root: Path, run) -> str:
    """An error that hit a full disk, said as that, with the advice."""
    return f"the disk is full ({' '.join(text.split())}): {advice(root, run)}"
