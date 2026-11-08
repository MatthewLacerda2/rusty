"""`tools/lint` on a merged tree before it is pushed, under `--no-check` too ([`Linter`], #1956).

## Two heads under the cap, one merge over it

`--no-check` (the batch default) skips the queue's compile of each updated head
and lets CI judge it, about 30 minutes later. On 2026-11-08 that cost two coder
rounds in one batch: #1905 and #1908 were each green alone, and merged together
`src/api/snapshot/components.rs` reached 302/300; #1950, green alone, merged with
the `main` #1942 and #1946 had landed on, left `src/app/play.rs` at 309/300, and
nothing said why until the orchestrator re-ran `tools/lint` by hand two hours
later. No textual merge sees either, and no brief can: the file was not near the
cap when the brief was written.

`tools/lint` needs no compile of rusty: it is a std-only crate of its own that
builds in seconds and runs every pass CI's `lint` job runs ([`PASSES`]) in a few.
So the queue runs it on the tree it is about to push: after `main` is merged into
a head ([`update.advance`]'s `check`), and on a merge train's tree before its
branch is pushed (`train.world.compiles`). A violation hands the head back with
the report's first lines and the merged pull requests that touched the file
([`blame`]), with no push and no CI round; a train is split, as a red one is.
Re-admission is by a push, like a conflict's.

## Built once per `tools/lint` tree

The binary is built from the merged tree's own `tools/lint` (what CI would run),
into a target directory inside the queue's worktree (`target/merge-queue-lint`,
never a shared `CARGO_TARGET_DIR`), and copied aside under that tree's hash, so
a later take with the same `tools/lint` runs it without asking cargo anything.
A pull request that changes `tools/lint` gets a build of its own.

## A broken local tool never stops the queue

No cargo, a build that fails, a pass that crashes (any exit but 0 or 1): the
queue says so once per `tools/lint` tree and carries on as before #1956, leaving
the tree to CI. Only a pass that ran and found something hands a head back.
"""

from __future__ import annotations

import functools
import os
import re
import shutil
import subprocess

from merging.queue import gitio, outcomes


# Every pass CI's `lint` job runs (`.github/workflows/lint.yml`), in its order.
# None compiles rusty; `--dead-pub` is a signal, never a gate, so it is not here.
PASSES: tuple[tuple[str, ...], ...] = (
    (), ("--rules",), ("--layers",), ("--determinism",), ("--direction",), ("--components",), ("--parity",),
)

# Where each pass writes its verdict, relative to the tree it ran in.
REPORT = os.path.join(".lint", "report.txt")

# How many of the report's lines a hand-back quotes, after its `lint: FAILED` line.
REPORT_HEAD = 12

# How many named files [`blame`] asks git about.
BLAMED_FILES = 3

# The pull request a commit came from: a squash merge's `(#N)` at the end of its
# subject, or a merge train's own `merge train: #N at <sha>`.
SQUASHED = re.compile(r"\(#(\d+)\)\s*$")
TRAIN_CAR = re.compile(r"^merge train: #(\d+) ")

# A path in a report line: something with a slash and a dot-extension.
PATH = re.compile(r"(?<![\w/.-])((?:[\w.-]+/)+[\w.-]+\.\w+)")

HANDED_BACK = (
    "Handed back unpushed: each side passed alone, and the merge does not (#1956)."
    " Fix it on the branch (a push re-admits it), naming the pull request above in the re-brief."
)


def both(first, second):
    """A `check(work)` running `first`, then `second` (when given) if `first` passed."""
    return first if second is None else functools.partial(chained, first, second)


def chained(first, second, work: str) -> list[str]:
    return first(work) or second(work)


def binary_name() -> str:
    return "lint.exe" if os.name == "nt" else "lint"


class Linter:
    """`tools/lint`, built per `tools/lint` tree into `target` and run on a merged tree.

    `runner` is `subprocess.run`'s shape and `say` [`outcomes.say`]'s, injected
    so the tests need no toolchain. One per watch ([`shared`]): its memory of
    built and failed trees is what makes a pass cost the run, not a build.
    """

    def __init__(self, target: str, runner=subprocess.run, say=outcomes.say):
        self.target, self.runner, self.say = target, runner, say
        self.built: dict[str, str] = {}  # tools/lint tree hash -> the binary built from it
        self.failed: set[str] = set()  # tools/lint tree hashes whose build failed, said once

    def tool_tree(self, work: str) -> str:
        """The hash of `work`'s `tools/lint` tree, `""` when it has none."""
        done = gitio.git("rev-parse", "--verify", "--quiet", "HEAD:tools/lint", cwd=work)
        return done.stdout.strip() if done.returncode == 0 else ""

    def binary(self, work: str) -> str | None:
        """The binary for `work`'s `tools/lint`, built if it is new; `None` when it
        cannot be (said once per tree: the queue then carries on without it)."""
        tree = self.tool_tree(work)
        if not tree:
            return None
        if tree in self.built and os.path.exists(self.built[tree]):
            return self.built[tree]
        if tree in self.failed:
            return None
        env = {**os.environ, "CARGO_TARGET_DIR": self.target}
        argv = ["cargo", "build", "--quiet", "--release", "--locked", "--manifest-path", "tools/lint/Cargo.toml"]
        try:
            done = self.runner(argv, cwd=work, env=env, capture_output=True, text=True, check=False)
            why = "" if done.returncode == 0 else (f"{done.stdout or ''}\n{done.stderr or ''}".strip().splitlines() or ["(no output)"])[-1]
        except OSError as error:  # no cargo on the PATH
            why = str(error)
        kept = os.path.join(self.target, "by-tree", tree[:16], binary_name())
        if not why:
            try:
                os.makedirs(os.path.dirname(kept), exist_ok=True)
                shutil.copy2(os.path.join(self.target, "release", binary_name()), kept)
            except OSError as error:
                why = f"could not keep the built binary: {error}"
        if why:
            self.failed.add(tree)
            self.say(f"could not build tools/lint ({why}); the merged tree is left to CI's lint job, as before #1956.")
            return None
        self.built[tree] = kept
        return kept

    def check(self, work: str, since: str, upto: str) -> list[str]:
        """Why the merged tree `work` must not be pushed, or `[]`.

        `since..upto` are the commits the merge brought in, for [`blame`]: an
        update's `head..origin/main`, a train's `origin/main..<tip>`.
        """
        exe = self.binary(work)
        if exe is None:
            return []
        for flags in PASSES:
            name = " ".join(("tools/lint", *flags))
            try:
                done = self.runner([exe, *flags], cwd=work, capture_output=True, text=True, check=False)
            except OSError as error:
                self.say(f"could not run {name} ({error}); the merged tree is left to CI.")
                return []
            if done.returncode == 0:
                continue
            if done.returncode != 1:
                tail = (f"{done.stdout or ''}\n{done.stderr or ''}".strip().splitlines() or ["(no output)"])[-1]
                self.say(f"{name} crashed (exit {done.returncode}: {tail}); the merged tree is left to CI.")
                return []
            said = report(work) or (f"{done.stdout or ''}\n{done.stderr or ''}".strip().splitlines())
            return [
                f"the merged tree fails {name}; not pushed.",
                *said[:REPORT_HEAD],
                *blame(work, said, since, upto),
                HANDED_BACK,
            ]
        return []


def report(work: str) -> list[str]:
    """The violations in `work`'s `.lint/report.txt`, its `lint: FAILED` line dropped."""
    try:
        with open(os.path.join(work, REPORT), encoding="utf-8") as text:
            lines = [line.rstrip() for line in text if line.strip()]
    except OSError:
        return []
    return [line for line in lines if not line.endswith(": FAILED")]


def pull_of(subject: str) -> str | None:
    """The pull request a commit subject names, `#N`, or `None`."""
    found = SQUASHED.search(subject) or TRAIN_CAR.search(subject)
    return f"#{found.group(1)}" if found else None


def blame(work: str, said: list[str], since: str, upto: str) -> list[str]:
    """One line per file the report names, saying which pull requests among
    `since..upto` touched it: the ones that brought it there, for the re-brief.
    Nothing when git cannot say: it is a hint, never the verdict."""
    files: list[str] = []
    for line in said:
        for path in PATH.findall(line):
            if path not in files and os.path.exists(os.path.join(work, path)):
                files.append(path)
    lines = []
    for path in files[:BLAMED_FILES]:
        done = gitio.git("log", "--format=%s", f"{since}..{upto}", "--", path, cwd=work)
        pulls = [] if done.returncode != 0 else [p for p in map(pull_of, done.stdout.splitlines()) if p]
        if pulls:
            lines.append(f"{path} was also changed by {', '.join(dict.fromkeys(pulls))}, merged since this branch's base.")
    return lines


# One [`Linter`] per target directory for the life of the process (the watch).
LINTERS: dict[str, Linter] = {}


def shared(target: str) -> Linter:
    """The watch's [`Linter`] for `target`, made on first use."""
    return LINTERS.setdefault(target, Linter(target))


def target_dir(root: str) -> str:
    """Where `tools/lint` is built: inside the queue's own worktree `root`."""
    return os.path.abspath(os.path.join(root, "target", "merge-queue-lint"))
