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
a head ([`update.advance`]'s `check`), on a head touching an agent doc that
`main` passed only by Markdown, merged just to lint ([`update.docs_merge_check`]),
and on a merge train's tree before its
branch is pushed (`train.world.compiles`). A violation hands the head back with
the report's first lines and the pull requests merged since its base that touched
the file ([`blame`]), with no push and no CI round; re-admission is by a push,
like a conflict's. A train's violation hands nothing back: its members are
requeued and the train split, as a red one is ([`SPLIT`]), with no blame lines.

## Built once per `tools/lint` tree

The binary is built from the merged tree's own `tools/lint` (what CI would run),
into a target directory inside the queue's worktree (`target/merge-queue-lint`,
never a shared `CARGO_TARGET_DIR`), and copied aside under that tree's hash, so
a later take with the same `tools/lint` runs it without asking cargo anything;
a relaunched watch finds that copy on disk and does not rebuild either.
A pull request that changes `tools/lint` gets a build of its own.

## A broken local tool never stops the queue

No cargo, a build that fails, a pass that crashes or cannot start (any exit but
0 or 1): the queue says so once per `tools/lint` tree, remembers it (`failed`,
`crashed`) and carries on as before #1956, leaving the tree to CI. A failure the
machine caused ([`machines`]: out of disk or memory, a killed process) is said
but never remembered, so the next take tries again: remembered, it would turn the
lint off for that tree until the watch relaunched (#1956's review). Only a pass
that ran and found something hands a head back.
"""

from __future__ import annotations

import functools
import os
import re
import shutil
import subprocess

from merging.queue import checks, gitio, outcomes


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

# A path in a report line: something with a slash and a dot-extension, or the
# root `CLAUDE.md` (the agent doc most likely to overflow, #2027's review).
PATH = re.compile(r"(?<![\w/.-])((?:[\w.-]+/)+[\w.-]+\.\w+|CLAUDE\.md)")

# The last line of an update's hand-back. It claims only what the queue checked:
# the merged tree fails, not that either side passed alone (#1956's review).
HANDED_BACK = "Handed back unpushed: the merged tree fails tools/lint (#1956). Fix it on the branch; a push re-admits it."

# The last line when a merge train's tree fails: nothing is handed back, the
# members are requeued and the train split, as a red one is (#1956's review).
SPLIT = (
    "The train's combined tree fails tools/lint (#1956): the train is split and each half tested alone;"
    " no member is handed back for it."
)


def both(first, second):
    """A `check(work)` running `first`, then `second` (when given) if `first` passed."""
    return first if second is None else functools.partial(chained, first, second)


def chained(first, second, work: str) -> list[str]:
    return first(work) or second(work)


def binary_name() -> str:
    return "lint"


def output(done) -> list[str]:
    """A finished process's stdout then stderr, as non-empty lines."""
    return [line for line in f"{done.stdout or ''}\n{done.stderr or ''}".splitlines() if line.strip()]


def machines(text: str, returncode: int = 0) -> bool:
    """Whether a failure is the machine's (#580's [`checks.ENVIRONMENT`]: out of
    disk or memory, a killed process), not the tool's: never remembered, so the
    next take tries again (#1956's review)."""
    return returncode < 0 or bool(checks.ENVIRONMENT.search(text))


class Linter:
    """`tools/lint`, built per `tools/lint` tree into `target` and run on a merged tree.

    `runner` is `subprocess.run`'s shape and `say` [`outcomes.say`]'s, injected
    so the tests need no toolchain. One per watch ([`shared`]): its memory of
    built, failed and crashed trees is what makes a pass cost the run, not a
    build, and a broken tool said once, not on every take.
    """

    def __init__(self, target: str, runner=subprocess.run, say=outcomes.say):
        self.target, self.runner, self.say = target, runner, say
        self.built: dict[str, str] = {}  # tools/lint tree hash -> the binary built from it
        self.failed: set[str] = set()  # tools/lint tree hashes whose build failed, said once
        self.crashed: set[str] = set()  # tools/lint tree hashes a pass crashed or could not start in, said once

    def tool_tree(self, work: str) -> str:
        """The hash of `work`'s `tools/lint` tree, `""` when it has none."""
        done = gitio.git("rev-parse", "--verify", "--quiet", "HEAD:tools/lint", cwd=work)
        return done.stdout.strip() if done.returncode == 0 else ""

    def binary(self, work: str, tree: str) -> str | None:
        """The binary for `work`'s `tools/lint` (tree hash `tree`), built if it is
        new; `None` when it cannot be (said once per tree, unless the machine
        failed: the queue then carries on without it)."""
        kept = os.path.join(self.target, "by-tree", tree[:16], binary_name())
        if tree in self.built and os.path.exists(self.built[tree]):
            return self.built[tree]
        if tree in self.failed:
            return None
        if os.path.exists(kept):  # built by an earlier watch: a relaunch does not rebuild
            self.built[tree] = kept
            return kept
        env = {**os.environ, "CARGO_TARGET_DIR": self.target}
        argv = ["cargo", "build", "--quiet", "--release", "--locked", "--manifest-path", "tools/lint/Cargo.toml"]
        machine = False
        try:
            done = self.runner(argv, cwd=work, env=env, capture_output=True, text=True, check=False)
            said = output(done)
            why = "" if done.returncode == 0 else (said or ["(no output)"])[-1]
            machine = done.returncode != 0 and machines("\n".join(said), done.returncode)
        except OSError as error:  # no cargo on the PATH, or no memory to start it
            why, machine = str(error), machines(str(error))
        if not why:
            try:
                os.makedirs(os.path.dirname(kept), exist_ok=True)
                shutil.copy2(os.path.join(self.target, "release", binary_name()), kept)
            except OSError as error:
                why, machine = f"could not keep the built binary: {error}", machines(str(error))
        if why:
            if machine:
                self.say(f"could not build tools/lint ({why}): the machine's failure; this tree is left to CI, and the next take tries again.")
            else:
                self.failed.add(tree)
                self.say(f"could not build tools/lint ({why}); the merged tree is left to CI's lint job, as before #1956.")
            return None
        self.built[tree] = kept
        return kept

    def unrun(self, tree: str, why: str, machine: bool) -> list[str]:
        """A pass that crashed or could not start: the tree is left to CI. Said
        once per `tools/lint` tree, unless it was the machine's failure."""
        if machine:
            self.say(f"{why}: the machine's failure; this tree is left to CI, and the next take tries again.")
        else:
            self.crashed.add(tree)
            self.say(f"{why}; this tools/lint is left to CI from now on.")
        return []

    def check(self, work: str, since: str | None = None, upto: str | None = None, last: str = HANDED_BACK) -> list[str]:
        """Why the merged tree `work` must not be pushed, or `[]`.

        The first line names the pass and its first violation (rule and file),
        since `make queue-events` shows only that one; `last` ends it
        ([`HANDED_BACK`] for an update, [`SPLIT`] for a train). `since..upto`
        are the commits the merge brought in, for [`blame`]: an update's
        `head..origin/main`. A train gives none: no line is blamed there.
        """
        tree = self.tool_tree(work)
        if not tree or tree in self.crashed:
            return []
        exe = self.binary(work, tree)
        if exe is None:
            return []
        for flags in PASSES:
            name = " ".join(("tools/lint", *flags))
            try:
                done = self.runner([exe, *flags], cwd=work, capture_output=True, text=True, check=False)
            except OSError as error:
                return self.unrun(tree, f"could not run {name} ({error})", machines(str(error)))
            if done.returncode == 0:
                continue
            if done.returncode != 1:
                text = output(done)
                why = f"{name} crashed (exit {done.returncode}: {(text or ['(no output)'])[-1]})"
                return self.unrun(tree, why, machines("\n".join(text), done.returncode))
            said = report(work) or output(done)
            first = f"the merged tree fails {name}: {said[0]}; not pushed." if said else f"the merged tree fails {name}; not pushed."
            blamed = blame(work, said, since, upto) if since and upto else []
            return [first, *said[1:REPORT_HEAD], *blamed, last]
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
