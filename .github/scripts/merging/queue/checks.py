"""The local checks a rebased head passes before it is pushed ([`verify`], #571).

## A clean rebase is not a compiling one

A merge ahead can change a signature the branch still calls, and no textual
merge sees it. On 2026-09-30 #542 removed a `Renderer::render` argument that
#538's new tests still passed; the rebase was clean and CI went red ten
minutes later — a queue round spent on a compile error (#571). A rebase of
#563 onto #558 and #564 pushed a file to 308 of 300 lines the same night. So
before pushing a rebase that moved the head, [`verify`] runs [`LOCAL_CHECKS`]
in the rebased tree: `cargo check --locked --all-targets` with `dev` and with
`--no-default-features`, then the size gate — a minute or two on a warm
target, against ten for the CI round it saves. A failure hands the branch
back unpushed: a head known to be broken is never pushed to find out again.
Skipped for a Markdown-only branch (nothing compiles differently), a rebase
that moved nothing, Dependabot, and `--no-check`.

Those builds land in **one target directory owned by the queue**
(`target/merge-queue` under `--root` by default), never a worktree's own.
CLAUDE.md's rule against a shared `CARGO_TARGET_DIR` is about worktrees
building *at the same time* into one directory; the queue is one process
taking one branch at a time, so its directory is only ever shared with its own
previous turn — which is what keeps it warm. They get half the cores
(`CARGO_BUILD_JOBS`, unless already set), because another agent's build may be
running beside them (#497).

## A full disk is not a broken branch

A check can fail for the machine's reasons rather than the code's. The first
real run of the pre-push check (2026-09-30, #580) ran from a worktree in the
session scratchpad — a 7.8 GB tmpfs — and the cold `target/merge-queue` build
filled it: `Disk quota exceeded (os error 122)`, and all three pull requests
were handed back as "a merge ahead changed something this branch relies on".
None was broken. So [`verify`] tells the two apart ([`ENVIRONMENT`]): out of
disk or memory, or rustc and the linker killed, **stops the whole queue**
([`stopping.Stopped`], [`stopping.drain`]) with one message naming the cause — every later
branch would fail the same way — and names the rest as not taken. Before any
check runs, [`preflight`] refuses a target directory on a tmpfs outright: run
the queue from a worktree under `.claude/worktrees/`, never the scratchpad.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
from pathlib import Path

from merging.queue import stopping
from merging.verdict import changes, github


# What a rebased tree must pass before it is pushed ([`verify`], #571),
# cheapest first. Each mirrors a gate CI blocks on.
LOCAL_CHECKS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("cargo check (dev)", ("cargo", "check", "--locked", "--all-targets", "--features", "dev")),
    ("cargo check (no default features)", ("cargo", "check", "--locked", "--all-targets", "--no-default-features")),
    ("the size gate", ("cargo", "run", "--quiet", "--locked", "--manifest-path", "tools/lint/Cargo.toml")),
)

# How much of a failed check's output the hand-back quotes: the end, where
# cargo puts the error and the lint its verdict.
OUTPUT_TAIL = 15

# What a check failing for the *machine's* reasons says, as opposed to the
# code's (#580): out of disk (ENOSPC, os error 28) or over quota (EDQUOT, os
# error 122), out of memory (ENOMEM, os error 12), or rustc or the linker
# killed by the OOM killer (signal 9).
ENVIRONMENT = re.compile(
    r"no space left on device|disk quota exceeded|os error (?:28|122|12)\b"
    r"|cannot allocate memory|memory allocation of \d+ bytes failed"
    r"|signal:? 9\b|sigkill|terminated with signal 9",
    re.IGNORECASE,
)

# Where the queue should run from instead of a tmpfs.
QUEUE_ROOT = "a worktree under .claude/worktrees/"


def verify(work: str, target: str, runner=subprocess.run) -> list[str]:
    """Run [`LOCAL_CHECKS`] in the rebased tree `work`; why not to push, or `[]`.

    Stops at the first failure and quotes its tail. A failure that is the
    machine's ([`ENVIRONMENT`]) raises [`stopping.Stopped`] instead. `runner` is
    `subprocess.run`'s shape, injected so the tests need no toolchain.
    """
    env = {**os.environ, "CARGO_TARGET_DIR": target}
    env.setdefault("CARGO_BUILD_JOBS", str(max(1, (os.cpu_count() or 2) // 2)))
    for name, argv in LOCAL_CHECKS:
        done = runner(list(argv), cwd=work, env=env, capture_output=True, text=True, check=False)
        if done.returncode != 0:
            text = f"{done.stdout or ''}\n{done.stderr or ''}".strip()
            said = text.splitlines()
            if ENVIRONMENT.search(text):
                raise stopping.Stopped([
                    f"the machine ran out of disk or memory during {name}; nothing is known to be wrong with this branch.",
                    *said[-OUTPUT_TAIL:],
                    room(target),
                    "Stopped the queue: every later branch would fail the same way."
                    f" Free space, or run it from {QUEUE_ROOT} (never the scratchpad), then queue them again.",
                ])
            return [
                f"the rebased head fails {name}; not pushed.",
                *said[-OUTPUT_TAIL:],
                "The rebase was textually clean, so a merge ahead changed"
                " something this branch relies on. Handed back: fix it on the"
                " branch, then queue it again.",
            ]
    return []


def existing(path: str) -> str:
    """`path`, or its nearest ancestor that exists (a cold target does not yet)."""
    path = os.path.abspath(path)
    while not os.path.exists(path) and os.path.dirname(path) != path:
        path = os.path.dirname(path)
    return path


def room(target: str) -> str:
    """How much disk is left where the checks build, for the stop message."""
    free = shutil.disk_usage(existing(target)).free
    return f"{free / 1e9:.1f} GB free under {target}."


def mount_type(path: str, mountinfo: str = "/proc/self/mountinfo") -> str | None:
    """The filesystem type `path` lives on, or `None` where the kernel does not
    say (macOS has no `/proc`). The longest mount point containing it wins."""
    try:
        lines = Path(mountinfo).read_text().splitlines()
    except OSError:
        return None
    real, best, kind = os.path.realpath(existing(path)), "", None
    for line in lines:
        fields = line.split()
        if "-" not in fields[4:]:
            continue
        point, fstype = fields[4], fields[fields.index("-", 4) + 1]
        inside = real == point or real.startswith(point.rstrip("/") + "/")
        if inside and len(point) >= len(best):
            best, kind = point, fstype
    return kind


def preflight(target: str) -> list[str]:
    """Why the checks cannot build in `target`, or `[]`.

    A tmpfs is refused outright: it is small and counts against RAM, and a cold
    build filled one (#580). Free space is not given a threshold here — a build's
    size is the crate's, not a constant — and running out is caught by [`verify`].
    """
    if mount_type(target) != "tmpfs":
        return []
    return [
        f"the checks would build in {target}, which is on a tmpfs; a cold build fills it (#580).",
        f"Run the queue from {QUEUE_ROOT}, or pass --target-dir on a real disk.",
    ]


def target_dir(opts: argparse.Namespace) -> str:
    """Where [`verify`] builds: the queue's own directory, never a worktree's."""
    return os.path.abspath(opts.target_dir or os.path.join(opts.root, "target", "merge-queue"))


def needs_check(pull: dict, opts: argparse.Namespace) -> bool:
    """Whether [`verify`] runs for this pull request's rebase."""
    return not (opts.no_check or opts.dry_run or changes.markdown_only(github.paths(pull)))
