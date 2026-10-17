"""`queue-daemon.py`'s command line, and [`supervisor.run`]'s effects bound to this machine."""

from __future__ import annotations

import argparse
import functools
import os
import subprocess
import sys
import time
from pathlib import Path

from merging.queue import gitio, world
from merging.watch import ending, freshness, lock, supervisor


def common_dir(root: str) -> Path:
    """The checkout's common git directory, which every worktree shares."""
    return Path(root, gitio.git("rev-parse", "--git-common-dir", cwd=root).stdout.strip() or ".git")


def update(root: str) -> str | None:
    """Move the checkout to `origin/main`, or say why not. A failed fetch is
    not a refusal: the watch runs the code it has and meets the network itself."""
    gitio.git("fetch", "--quiet", "origin", "main", cwd=root)
    if gitio.git("status", "--porcelain", "--untracked-files=no", cwd=root).stdout.strip():
        return f"{root} has uncommitted changes; run the supervisor from a worktree of its own."
    moved = gitio.git("checkout", "--quiet", "--detach", "origin/main", cwd=root)
    return None if moved.returncode == 0 else f"could not check out origin/main: {moved.stderr.strip()}"


def watch(root: str, args: list[str]) -> tuple[int, str]:
    """One watch, its output passed through line by line: its exit status, and
    what followed `watch ended: `."""
    ended = ""
    with subprocess.Popen(
        [sys.executable, str(Path(root, ".github", "scripts", "merge-queue.py")), "--watch", *args],
        cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
    ) as proc:
        for line in proc.stdout:
            print(line, end="", flush=True)
            if "watch ended: " in line:
                ended = line.split("watch ended: ", 1)[1].strip()
    return proc.returncode, ended


def effects(root: str, forwarded: list[str], argv: list[str]) -> argparse.Namespace:
    """What [`supervisor.run`] does to the world. Called once, at start: that is
    when the supervisor's own code is noted ([`freshness.launched`])."""
    code = freshness.launched(freshness.running(), world.git_at)
    return argparse.Namespace(
        stop_requested=lambda: world.consume(common_dir(root) / ending.STOP_FILE),
        update=lambda: update(root),
        restart_needed=lambda: bool(freshness.changed(code, functools.partial(world.merge_tree_git, root))),
        restart=lambda: os.execv(sys.executable, [sys.executable, sys.argv[0], *argv]),
        watch=lambda: watch(root, ["--root", root, *forwarded]),
        sleep=time.sleep,
        say=lambda line: print(line, flush=True),
    )


def parse(argv: list[str]) -> tuple[argparse.Namespace, list[str]]:
    parser = argparse.ArgumentParser(
        prog="queue-daemon.py",
        description="Keep `merge-queue.py --watch` running across its planned exits (#908). Flags it does"
        " not know are passed to the watch, e.g. --no-check --since 2026-10-16T19:00.",
    )
    parser.add_argument("--detach", action="store_true", help="start in the background, logging to .git/" + supervisor.LOG)
    parser.add_argument("--stop", action="store_true", help="ask the watch and its supervisor to stop between pull requests")
    parser.add_argument("--root", default=".", metavar="DIR", help="the checkout to run in; moved to origin/main before each watch")
    opts, forwarded = parser.parse_known_args(argv)
    for flag in ("--watch", "--dry-run", "--for"):
        if any(arg == flag or arg.startswith(flag + "=") for arg in forwarded):
            parser.error(f"{flag} is not for a supervised watch: it adds --watch itself, and a detached watch needs no deadline")
    return opts, forwarded


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    opts, forwarded = parse(argv)
    root = os.path.abspath(opts.root)
    if opts.stop:
        path = common_dir(root) / ending.STOP_FILE
        path.parent.mkdir(parents=True, exist_ok=True)
        path.touch()
        print(f"queue-daemon: asked the watch to stop ({path}); it stops between pull requests.")
        return 0
    if opts.detach:
        log = common_dir(root) / supervisor.LOG
        log.parent.mkdir(parents=True, exist_ok=True)
        with open(log, "a") as out:
            child = subprocess.Popen(
                [sys.executable, sys.argv[0], *(a for a in argv if a != "--detach")],
                stdout=out, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True,
            )
        print(f"queue-daemon: supervisor started, PID {child.pid}; log {log}; stop with `make queue-stop`.")
        return 0
    held, holder = lock.hold(common_dir(root) / supervisor.LOCK)
    if held is None:
        print(f"queue-daemon: another supervisor runs (PID {holder}); one at a time.", flush=True)
        return lock.LOCKED_STATUS
    with held:
        return supervisor.run(effects(root, forwarded, argv))
