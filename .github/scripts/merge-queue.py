#!/usr/bin/env python3
"""`merge-queue.py N M ...` — do the waiting that serialized merging costs.

Merging stays serialized, one branch at a time, because rusty is one compiled
crate: two pull requests can each be green alone and break `main` together (a
rename, a changed signature, a moved module). Nothing here proposes otherwise.
What this automates is **who sits through the ten minutes** — rebase the
branch, compile-check the rebased tree ([`verify`]), force-push it, wait for the runs on the rebased head, ask
[`mergeable.judge`], squash-merge, take the next one. Without it an agent sits
through every cold CI run of a batch holding a worktree open. Ported from
scorsese (#486); its incidents are cited as scorsese#N.

Invoked, never a service: it runs when somebody types `make queue`, on the pull
requests they name, in the order they name them. `--dry-run` does every read
and the local rebase, and writes nothing to GitHub. `--watch` (#664) names
none: it takes each pull request as it turns ready, in label-priority order,
and is still invoked, never a service — a hand-back skips that pull request
and it carries on (#751); it exits on the machine failing, when nothing is
left, or between pull requests once its `--for` deadline passes
([`queue_watch`]).

## Why it does not skip a run instead

scorsese#492 weighed a cheaper answer — skip the fresh run when the rebase
"changed nothing that matters". A run's verdict is about a **tree**, and it
carries only to the same tree; "main's delta touched no module the branch did"
is not that, and in one crate it is almost never even true. Measured over the
batch that motivated it, 23 re-runs, 0 of them on an already-tested tree. So
the one sound skip is kept: **a rebase that moves nothing** — the branch is
already on `main`'s tip, the run on record is a run on this exact commit
([`push_needed`]).

**And one that could move nothing that compiles.** When every commit `main`
gained since the branch's base touches only Markdown — [`mergeable.inert`],
so never `docs/api/`, which tests parse — the head is left where
it is: no rebase, no push, and the run on record stands ([`docs_advance`],
#587). That is not the per-module guess above: those commits cannot change a
single compiled or tested byte, which is why CLAUDE.md lets them merge
unserialized. On 2026-09-30 merging the Markdown-only #586 cost #584, waiting
behind it, a rebase and a ten-minute CI round. The squash merge still lands on
the newer `main`; a branch editing the same `.md` file gets `gh pr merge`'s
conflict, handed back as any refused merge is.

## The three things it must not do

1. **It never resolves a conflict.** Empty conflict set, or it hands the branch
   back naming the paths. rusty's conflict hot spots — `ComponentKind::ALL`'s
   hard-coded length, `app/registry.rs`'s system order — are exactly the ones
   where keeping both sides is wrong and no textual merge knows it.
2. **It never merges on a local result.** A local check may only *refuse* a
   push, never grant a merge. `make gates` is the author's job before
   readying; CI is the cross-platform claim (the macOS runner), and the only
   thing a merge consults is [`mergeable.judge`].
3. **It never reads the mutation or coverage signal.** Signals never hold a
   merge.

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
([`Stopped`], [`drain`]) with one message naming the cause — every later
branch would fail the same way — and names the rest as not taken. Before any
check runs, [`preflight`] refuses a target directory on a tmpfs outright: run
the queue from a worktree under `.claude/worktrees/`, never the scratchpad.

## What it touches, and what it leaves alone

The rebase happens in a **throwaway, detached worktree this script creates and
removes** under the system temp dir — never in a worktree somebody is working
in. The push is
`--force-with-lease` against the head the pull request had when its turn
began, so a push from anywhere else refuses rather than being overwritten. The
merge is `--match-head-commit`, so GitHub refuses it if the head moved after
the verdict.

A merged branch's worktree under `.claude/worktrees/` is **not** removed and
no branch is deleted — that is 12–16 GB and a checkout somebody may be
standing in. The summary names them instead.

**Dependabot rebases itself** (and cancels its own runs as it does). A
force-push from here would make Dependabot stop maintaining the branch, so a
bot pull request that is behind is asked to rebase — `@dependabot rebase` — and
the queue waits for a head that sits on `main`'s tip ([`bot_head`]), then
judges that head like any other.

## A hand-back skips the entry; it does not stop the queue

Only the machine does (see above). Every branch is rebased onto `main` as `main` is at its own turn, and CI
judges that tree, so nothing a hand-back leaves behind can make a later merge
unsound (scorsese#495). Each hand-back is named in the summary, and the exit
status is non-zero if there was any.

## GitHub's answer about a state is not always the state

The runs listing is eventually consistent ([`progress`]'s grace), a force-push
lags its own poll ([`head_state`]'s), and a merge call can answer **502 having
already merged** (scorsese#495). So a failed merge is sorted by what failed: a
transport failure is followed by asking whether it merged ([`transport`],
[`landed`]); a refusal GitHub reasoned about is handed back with no second
call.

Run it:

    make queue PRS="524 526"
    make queue ARGS="--watch --no-check --for 110"
    python3 .github/scripts/merge-queue.py 524 526 --dry-run

The decisions are pure functions over plain dictionaries, tested without a
network (`make scripts`).
"""

from __future__ import annotations

import argparse
import functools
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

_spec = importlib.util.spec_from_file_location(
    "mergeable", Path(__file__).resolve().parent / "mergeable.py"
)
mergeable = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(mergeable)

_spec = importlib.util.spec_from_file_location(
    "queue_watch", Path(__file__).resolve().parent / "queue_watch.py"
)
queue_watch = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(queue_watch)

# How often GitHub is asked again. A cold run is about ten minutes; a tighter
# poll buys only API calls.
POLL_SECONDS = 30

# How long "nothing has built this yet" is read as *not yet*. A push and the
# runs it creates are not simultaneous, and the listing is eventually
# consistent; after this, the silence is the answer ([`mergeable.no_run`]).
RUN_APPEARS_SECONDS = 300

# The ceiling on one branch, in minutes: well past a cold run (the macOS and
# Windows jobs are the slow ones), because giving up early hands back a branch
# about to go green. Reaching it is a hand-back, never a merge.
DEADLINE_MINUTES = 45

# How long a merge call that failed in transit is given to show up as merged.
MERGE_SETTLES_SECONDS = 60

# What a failure *in transit* looks like in `gh`'s stderr, as opposed to a
# refusal: a REST 5xx, a GraphQL `HTTP 5xx`, or Go's network errors verbatim.
TRANSPORT_STATUS = re.compile(r"(status code:|HTTP)\s*5\d\d\b", re.IGNORECASE)
TRANSPORT_WORDS = (
    "timeout",
    "timed out",
    "deadline exceeded",
    "connection reset",
    "connection refused",
    "broken pipe",
    "unexpected eof",
)

# How `gh pr view --json author` names Dependabot, and the command it obeys.
BOTS = ("app/dependabot", "dependabot[bot]", "dependabot")
BOT_REBASE = "@dependabot rebase"

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

# The commits `main` gained since a branch's base, with every path each touched.
# `--no-renames`, so a code file renamed to `.md` shows its old path too.
MAIN_GAINED = ("log", "--no-renames", "--name-only", "--format=>%H", "origin/main")

# What a head left alone by [`docs_advance`] reports instead of a push.
DOCS_ONLY = "`main` moved only by Markdown; not rebased, the run on record stands (#587)"

WAIT, GO, STOP = "wait", "go", "stop"

# What the summary calls each ending. Merged, green and dry are separate so the
# report never claims a merge it did not make.
MERGED, GREEN, DRY, HANDED_BACK = "merged", "green", "dry run", "handed back"
# The machine failed, not the branch ([`Stopped`]); and what the stop left untouched.
STOPPED, NOT_TAKEN = "stopped the queue", "not taken"
# A watched pull request that turned draft or closed between listing and taking,
# and what [`take`] says about one.
NOT_READY = "not ready"
NOT_OPEN, A_DRAFT = "it is {}.", "it is a draft; mark it ready first."


class Stopped(Exception):
    """A failure that is the machine's, not the branch's: the queue stops.

    Raised through [`advance`] (whose `finally` still removes its worktree) and
    caught by [`drain`], which names every later entry as not taken.
    """

    def __init__(self, lines: list[str]):
        super().__init__(lines[0])
        self.lines = lines


def git(*args: str, cwd: str | None = None) -> subprocess.CompletedProcess:
    """`git`, captured and never fatal. Callers read `returncode` themselves."""
    return subprocess.run(["git", *args], capture_output=True, text=True, check=False, cwd=cwd)


def ordered(numbers: list[int]) -> list[int]:
    """The queue as given, repeats dropped, order kept — a repeat is a typo."""
    seen: set[int] = set()
    return [n for n in numbers if not (n in seen or seen.add(n))]


def conflicts(unmerged: str) -> list[str]:
    """The paths a failed rebase left conflicted, from `--diff-filter=U`."""
    return [line.strip() for line in unmerged.splitlines() if line.strip()]


def push_needed(before: str, after: str) -> bool:
    """Whether the rebase moved the head commit — and so whether to push.

    If not, the branch was already on `main`'s tip and the run on record is a
    run on this exact commit; pushing would buy a second cold run of it.
    """
    return before != after


def docs_advance(log: str) -> bool:
    """Whether `main` moved past the head, and only by [`mergeable.inert`] paths.

    `log` is [`MAIN_GAINED`]'s output: a `>sha` line per commit, then its paths.
    No commit is not an advance — the rebase is a no-op either way.
    """
    lines = [line.strip() for line in log.splitlines() if line.strip()]
    commits = [line for line in lines if line.startswith(">")]
    return bool(commits) and mergeable.inert([line for line in lines if not line.startswith(">")])


def main_moved_by_docs(head: str, root: str) -> bool:
    """[`docs_advance`] over the commits on `origin/main` that `head` lacks.

    A git failure is *no*: the queue then rebases as it always has.
    """
    done = git(*MAIN_GAINED, "--not", head, cwd=root)
    return done.returncode == 0 and docs_advance(done.stdout)


def is_bot(pull: dict) -> bool:
    """Whether Dependabot owns this branch (and so rebases it itself)."""
    return (pull.get("author") or {}).get("login") in BOTS


def subject(pull: dict) -> str:
    """The squash commit's title: the PR title carrying `(#N)`, once."""
    title, tag = pull.get("title", "").strip(), f"(#{pull.get('number')})"
    return title if title.endswith(tag) else f"{title} {tag}"


def head_state(seen: str, pushed: str, before: str, waited: float) -> tuple[str, list[str]]:
    """Whether the head GitHub reports is the one this queue is watching.

    The pushed head goes; the pre-push head waits while the grace lasts
    (GitHub's view lags the push); a third sha is somebody else and stops —
    merging a commit this queue never watched is not given the benefit of the
    doubt.
    """
    if seen == pushed:
        return GO, []
    if seen == before and waited < RUN_APPEARS_SECONDS:
        return WAIT, [f"GitHub still reports {before[:7]}; it has not caught up with the push yet."]
    return STOP, [
        f"the head is {seen[:7]}, not the {pushed[:7]} this queue is watching.",
        "Somebody else pushed. Handed back rather than merging a commit this"
        " queue never watched.",
    ]


def sighted(pull: dict, runs: list[dict]) -> set[str]:
    """The workflows with a run on the head that GitHub has shown, by the runs
    listing or by the pull request's own checks ([`mergeable.in_rollup`])."""
    sha = pull.get("headRefOid", "")
    return {
        w for w in mergeable.WORKFLOWS if mergeable.runs_for(runs, sha, w) or mergeable.in_rollup(pull, w)
    }


def progress(
    pull: dict, runs: list[dict], jobs: dict[int, list[dict]], files: list[str], waited: float,
    seen: frozenset[str] | set[str] = frozenset(),
) -> tuple[str, list[str]]:
    """Whether to wait, merge, or hand this branch back — and why.

    The verdict is [`mergeable.judge`]; this adds what a polling loop needs and
    a one-shot check does not: *not yet* against *no*. A failure anywhere stops
    at once — a red run is not eventually consistent. A run in flight waits.
    A workflow in `seen` ([`sighted`] on an earlier poll) that is absent now
    is the listing flickering, and waits until the deadline (#592). Any other
    absence — no run, or only runs that built nothing — waits out
    [`RUN_APPEARS_SECONDS`], because the commit was pushed seconds ago and
    scorsese watched the runs listing omit a live run beside a skipped one.
    """
    if pull.get("isDraft"):
        return STOP, ["the pull request is a draft.", "CI does not check drafts. Mark it ready before queueing it."]

    sha = pull.get("headRefOid", "")
    states = [
        mergeable.assess(pull, w, mergeable.runs_for(runs, sha, w), jobs) for w in mergeable.WORKFLOWS
    ]
    for state, lines in states:
        if state == mergeable.FAILED:
            return STOP, lines
    for state, lines in states:
        if state == mergeable.RUNNING:
            return WAIT, lines
    for w, (state, _) in zip(mergeable.WORKFLOWS, states):
        if state == mergeable.ABSENT and w in seen:
            return WAIT, [
                f"the `{w}` run seen on {sha[:7]} has dropped out of the runs listing.",
                "The listing is eventually consistent and flickers; a run seen once"
                " is not gone (#592). Waiting for it to come back.",
            ]
    if any(state in (mergeable.ABSENT, mergeable.UNBUILT) for state, _ in states):
        if waited < RUN_APPEARS_SECONDS:
            return WAIT, [f"nothing has built {sha[:7]} yet.", "Ordinary this soon after a push, and not yet an answer."]

    ok, lines = mergeable.judge(pull, runs, jobs, files)
    return (GO if ok else STOP), lines


def transport(stderr: str) -> bool:
    """Whether a failed merge call failed *in transit* rather than being refused.

    Errs towards *refusal*: an unrecognised message is handed back without a
    second look, which never reports a merge as done that was not.
    """
    lowered = stderr.lower()
    return bool(TRANSPORT_STATUS.search(stderr)) or any(w in lowered for w in TRANSPORT_WORDS)


def landed(pull: dict) -> bool:
    """Whether GitHub's record says it merged. Not ancestry: a squash is a new
    commit, so the branch head is never an ancestor of `main`."""
    return pull.get("state") == "MERGED" or bool(pull.get("mergedAt"))


def settled(pull: dict | None, waited: float) -> tuple[str, list[str]]:
    """After a merge call failed in transit: merged, not yet known, or unknown.

    `None` is GitHub not answering either — no more an answer than the 5xx.
    Past the window it is handed back as *unknown*, never as refused: "refused"
    about a merge that happened invites a second one.
    """
    if pull is not None and landed(pull):
        return GO, ["the merge call failed in transit, but it merged."]
    if waited < MERGE_SETTLES_SECONDS:
        return WAIT, ["the merge call failed in transit; asking whether it merged."]
    return STOP, [
        "the merge call failed in transit and GitHub has not said it merged.",
        "Whether it did is unknown: look at the pull request before merging it again.",
    ]


def summary(results: list[tuple[int, str, str]]) -> list[str]:
    """The report — the only thing an unattended run leaves behind.

    A reason on every line, merges included, and the cleanup named rather than
    done (see the module doc).
    """
    lines = [f"#{number}: {state} — {why}" for number, state, why in results]
    merged = [n for n, state, _ in results if state == MERGED]
    if merged:
        lines.append(
            "Merged: "
            + ", ".join(f"#{n}" for n in merged)
            + ". Their worktrees (.claude/worktrees/) and branches are still here —"
            " remove the worktrees and delete the branches when nobody is standing in one."
        )
    return lines


def say(line: str, *rest: str) -> None:
    print(f"queue: {line}", flush=True)
    for extra in rest:
        print(f"  {extra}", flush=True)


def look(number: int) -> dict:
    """The pull request, in the fields every step below reads."""
    return mergeable.gh(
        "pr", "view", str(number), "--json",
        mergeable.PULL_FIELDS + ",headRefName,state,title,author",
    )


def merge_record(number: int) -> dict | None:
    """The merge fields, or `None` if GitHub did not answer — asked during the
    very outage that made it necessary, so never through the fatal `gh`."""
    done = subprocess.run(
        ["gh", "pr", "view", str(number), "--json", "state,mergedAt"],
        capture_output=True, text=True, check=False,
    )
    if done.returncode != 0:
        return None
    try:
        return json.loads(done.stdout)
    except json.JSONDecodeError:
        return None


def confirm(number: int, poll: float) -> tuple[str, list[str]]:
    """Ask, until [`settled`] says something other than *wait*."""
    began = time.monotonic()
    while True:
        state, lines = settled(merge_record(number), time.monotonic() - began)
        if state != WAIT:
            return state, lines
        say(f"#{number}: {lines[0]}")
        time.sleep(min(poll, MERGE_SETTLES_SECONDS / 4))


def verify(work: str, target: str, runner=subprocess.run) -> list[str]:
    """Run [`LOCAL_CHECKS`] in the rebased tree `work`; why not to push, or `[]`.

    Stops at the first failure and quotes its tail. A failure that is the
    machine's ([`ENVIRONMENT`]) raises [`Stopped`] instead. `runner` is
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
                raise Stopped([
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
    return not (opts.no_check or opts.dry_run or mergeable.markdown_only(mergeable.paths(pull)))


def on_tip(sha: str, root: str) -> bool:
    """Whether `sha` already contains `origin/main`'s tip (fetched by the caller)."""
    git("fetch", "--quiet", "origin", sha, cwd=root)
    return git("merge-base", "--is-ancestor", "origin/main", sha, cwd=root).returncode == 0


def advance(
    branch: str, head: str, root: str, push: bool, check=None
) -> tuple[str | None, list[str]]:
    """Put `branch` on `main`'s tip. Returns the new head, or why not.

    In a detached worktree this creates and removes, never one an agent may be
    standing in. The push goes from inside it, leased to `head`. With
    `push=False` (the dry run) the rebase is only computed. `check`, given the
    rebased tree's path, returns why not to push it ([`verify`]); it runs only
    when the rebase moved the head. A head `main` passed only by Markdown is
    returned as it is, with [`DOCS_ONLY`] as its note ([`docs_advance`]).
    """
    if main_moved_by_docs(head, root):
        return head, [f"{DOCS_ONLY}."]
    with tempfile.TemporaryDirectory(prefix="rusty-queue-") as tmp:
        work = os.path.join(tmp, "wt")
        made = git("worktree", "add", "--detach", work, head, cwd=root)
        if made.returncode != 0:
            return None, [f"could not check {branch} out: {made.stderr.strip()}"]
        try:
            done = git("rebase", "origin/main", cwd=work)
            if done.returncode != 0:
                unmerged = git("diff", "--name-only", "--diff-filter=U", cwd=work).stdout
                git("rebase", "--abort", cwd=work)
                named = conflicts(unmerged)
                return None, [
                    f"{branch} conflicts with `main`.",
                    f"Conflicted: {', '.join(named)}." if named else "The rebase stopped; git did not name a path.",
                    "Handed back unresolved: a machine that cannot say why the"
                    " code is shaped as it is does not get to pick a side.",
                ]
            fresh = git("rev-parse", "HEAD", cwd=work).stdout.strip()
            if check is not None and push_needed(head, fresh):
                broken = check(work)
                if broken:
                    return None, broken
            if push and push_needed(head, fresh):
                pushed = git(
                    "push", f"--force-with-lease=refs/heads/{branch}:{head}",
                    "origin", f"HEAD:refs/heads/{branch}", cwd=work,
                )
                if pushed.returncode != 0:
                    return None, [
                        f"the force-push of {branch} was refused: {pushed.stderr.strip()}",
                        "The lease held the head this queue started from, so"
                        " something else has pushed since. Handed back.",
                    ]
            return fresh, []
        finally:
            git("worktree", "remove", "--force", work, cwd=root)


def bot_head(number: int, head: str, root: str, opts: argparse.Namespace) -> tuple[str | None, list[str]]:
    """Dependabot's own rebase, waited for: a head on `main`'s tip, or why not.

    Never force-pushed from here — Dependabot stops maintaining a branch
    somebody else pushed to. Asked once with [`BOT_REBASE`], then polled.
    """
    if on_tip(head, root) or main_moved_by_docs(head, root):
        return head, []
    if opts.dry_run:
        return head, [f"would comment `{BOT_REBASE}` and wait for Dependabot's rebase."]
    # Not the fatal `gh`: if the comment fails, the wait below still answers.
    subprocess.run(["gh", "pr", "comment", str(number), "--body", BOT_REBASE], capture_output=True, check=False)
    began = time.monotonic()
    while time.monotonic() - began < opts.deadline * 60:
        time.sleep(opts.poll)
        seen = look(number).get("headRefOid", "")
        if seen != head and on_tip(seen, root):
            return seen, []
        say(f"#{number}: waiting for Dependabot to rebase {head[:7]}.")
    return None, [f"Dependabot did not rebase {head[:7]} onto `main` within {opts.deadline:.0f} minutes."]


def wait_for(
    repo: str, number: int, sha: str, before: str, deadline: float, poll: float
) -> tuple[str, list[str]]:
    """Poll until the runs on `sha` settle, or the deadline says stop.

    Never treats an absent check as a settled one — [`progress`] tells them
    apart, and [`head_state`] does the same one level up. What has been
    [`sighted`] on `sha` is remembered across polls, so a run the listing
    drops is waited for rather than declared lost (#592).
    """
    began, seen = time.monotonic(), set()
    while True:
        pull = look(number)
        waited = time.monotonic() - began
        state, lines = head_state(pull.get("headRefOid", ""), sha, before, waited)
        if state == STOP:
            return state, lines
        if state == GO:
            runs, jobs = mergeable.evidence(repo, sha)
            seen |= sighted(pull, runs)
            state, lines = progress(pull, runs, jobs, mergeable.paths(pull), waited, seen)
            if state != WAIT:
                return state, lines
        if waited > deadline:
            return STOP, [
                f"still waiting on {sha[:7]} after {deadline / 60:.0f} minutes.",
                *lines,
                "Handed back with the run still out. Nothing is red; nothing is green either.",
            ]
        say(f"#{number}: {lines[0]}")
        time.sleep(poll)


def dry(repo: str, pull: dict, fresh: str, notes: list[str]) -> tuple[int, str, str]:
    """The dry run's report: what would happen, and the verdict on today's head."""
    number, head = pull["number"], pull["headRefOid"]
    if notes:
        # Dependabot's branch, behind `main` ([`bot_head`]), or a head `main`
        # passed only by Markdown ([`advance`]): the note says what happens.
        moved, notes = notes[0].rstrip("."), notes[1:]
    elif push_needed(head, fresh):
        moved = f"would push {fresh[:7]} once it passes the local checks, then wait for CI on it"
    else:
        moved = "already on `main`; nothing to push"
    runs, jobs = mergeable.evidence(repo, head)
    ok, lines = mergeable.judge(pull, runs, jobs, mergeable.paths(pull))
    say(f"#{number}: {moved}.", *notes, f"today's head {head[:7]}: {'mergeable' if ok else 'not mergeable'} — {lines[0]}", *lines[1:])
    return number, DRY, f"{moved}; head {head[:7]} is {'mergeable' if ok else 'not mergeable'}."


def take(repo: str, number: int, opts: argparse.Namespace) -> tuple[int, str, str]:
    """One pull request, from where it is to merged or handed back."""
    pull = look(number)
    if pull.get("state") != "OPEN":
        return number, HANDED_BACK, NOT_OPEN.format(str(pull.get("state")).lower())
    if pull.get("isDraft"):
        # Before any push: a draft is not a claim, so there is nothing to check.
        return number, HANDED_BACK, A_DRAFT

    branch, head = pull["headRefName"], pull["headRefOid"]
    git("fetch", "--quiet", "origin", "main", cwd=opts.root)
    git("fetch", "--quiet", "origin", head, cwd=opts.root)
    if is_bot(pull):
        say(f"#{number} ({branch}): Dependabot's branch; it rebases itself.")
        fresh, notes = bot_head(number, head, opts.root, opts)
    else:
        say(f"#{number} ({branch}): rebasing {head[:7]} onto origin/main.")
        check = None
        if needs_check(pull, opts):
            check = functools.partial(verify, target=target_dir(opts))
        fresh, notes = advance(branch, head, opts.root, push=not opts.dry_run, check=check)
    if fresh is None:
        say(f"#{number}: {notes[0]}", *notes[1:])
        return number, HANDED_BACK, notes[0]
    if opts.dry_run:
        return dry(repo, pull, fresh, notes)

    if push_needed(head, fresh):
        say(f"#{number}: now at {fresh[:7]}; waiting for CI.")
    else:
        say(f"#{number}: {notes[0] if notes else 'already on `main`; the run on record is a run on it.'}")

    state, lines = wait_for(repo, number, fresh, head, opts.deadline * 60, opts.poll)
    say(f"#{number}: {lines[0]}", *lines[1:])
    if state != GO:
        return number, HANDED_BACK, lines[0]
    if opts.no_merge:
        return number, GREEN, lines[0]

    done = subprocess.run(
        ["gh", "pr", "merge", str(number), "--squash",
         "--match-head-commit", fresh, "--subject", subject(pull)],
        capture_output=True, text=True, check=False,
    )
    if done.returncode != 0:
        failure = done.stderr.strip()
        if not transport(failure):
            blocked = f"CI passed but the merge was refused: {failure}"
            say(f"#{number}: {blocked}")
            return number, HANDED_BACK, blocked
        say(f"#{number}: the merge call failed in transit: {failure}")
        state, after = confirm(number, opts.poll)
        say(f"#{number}: {after[0]}", *after[1:])
        if state != GO:
            return number, HANDED_BACK, after[0]
    say(f"#{number}: merged.")
    return number, MERGED, lines[0]


def parse(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="merge-queue.py",
        description=(
            "Rebase, push, wait for CI and squash-merge each pull request in"
            " turn. Merging stays serialized; this only does the waiting."
        ),
    )
    parser.add_argument("prs", metavar="PR", type=int, nargs="*", help="pull request numbers, merged in the order given")
    parser.add_argument(
        "--watch", action="store_true",
        help=(
            "name no PR: take each one as it turns ready, by label priority; a hand-back is skipped and announced"
            f" ('{queue_watch.HANDED_BACK_LINE} #N'); exit when nothing is left: 1 if anything was handed back,"
            f" {queue_watch.MACHINE_STATUS} if the machine or GitHub failed"
        ),
    )
    parser.add_argument(
        "--idle", type=float, default=queue_watch.IDLE_MINUTES, metavar="MINUTES",
        help=f"--watch: exit when nothing is ready and no open PR has moved for this long (default {queue_watch.IDLE_MINUTES})",
    )
    parser.add_argument(
        "--for", dest="for_minutes", type=float, metavar="MINUTES",
        help=(
            "--watch: take no new PR once this long has passed; finish the one in hand, then exit cleanly,"
            f" as 'nothing left' does, with a last line saying '{queue_watch.DEADLINE}' (relaunch it then)"
        ),
    )
    parser.add_argument("--no-merge", action="store_true", help="stop at green and hand each branch back rather than merging it")
    parser.add_argument(
        "--dry-run", action="store_true",
        help="read everything and rebase locally, but push, comment and merge nothing; judges each head as it is today",
    )
    parser.add_argument(
        "--deadline", type=float, default=DEADLINE_MINUTES, metavar="MINUTES",
        help=f"give up waiting on one branch after this long (default {DEADLINE_MINUTES})",
    )
    parser.add_argument(
        "--poll", type=float, default=POLL_SECONDS, metavar="SECONDS",
        help=f"how often to ask GitHub again (default {POLL_SECONDS})",
    )
    parser.add_argument(
        "--no-check", action="store_true",
        help="push a rebased head without compile-checking it first (CI still judges it)",
    )
    parser.add_argument(
        "--target-dir", metavar="DIR",
        help="where the pre-push checks build (default: target/merge-queue under --root; never a worktree's own)",
    )
    parser.add_argument("--root", default=".", metavar="DIR", help="the git checkout to rebase in (default: the current directory)")
    opts = parser.parse_args(argv)
    if bool(opts.prs) == opts.watch:
        parser.error("name the pull requests, or pass --watch; not both, not neither")
    if opts.for_minutes is not None and not opts.watch:
        parser.error("--for bounds a --watch; named pull requests end on their own")
    return opts


def drain(numbers: list[int], turn) -> list[tuple[int, str, str]]:
    """Each pull request's `turn` in order, until the machine fails ([`Stopped`]).

    A hand-back carries on to the next; a stop names the one it happened on
    and every later one as not taken, and takes no more.
    """
    results = []
    for at, number in enumerate(numbers):
        try:
            results.append(turn(number))
        except Stopped as stop:
            say(f"#{number}: {stop.lines[0]}", *stop.lines[1:])
            results.append((number, STOPPED, stop.lines[0]))
            results += [(n, NOT_TAKEN, f"the queue stopped at #{number}.") for n in numbers[at + 1:]]
            break
    return results


def quietly(*args: str) -> object | None:
    """`gh`'s parsed JSON, or `None` when it fails: a watch outlives a blip."""
    done = subprocess.run(["gh", *args], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        say(f"gh {args[0]} {args[1]} failed: {done.stderr.strip()}")
        return None
    try:
        return json.loads(done.stdout)
    except json.JSONDecodeError:
        return None


def issue_labels() -> dict[int, set[str]]:
    """Every open issue's labels: the priority [`queue_watch.rank`] reads."""
    issues = quietly("issue", "list", "--state", "open", "--limit", "1000", "--json", "number,labels") or []
    return {i["number"]: queue_watch.names(i.get("labels")) for i in issues}


def unready(result: tuple[int, str, str]) -> tuple[int, str, str]:
    """[`take`]'s refusal of a draft or closed pull request, renamed for the
    watch: it raced the listing, and is not a hand-back to wake anyone for."""
    number, state, why = result
    refused = why == A_DRAFT or why in {NOT_OPEN.format(s) for s in ("closed", "merged")}
    return (number, NOT_READY, why) if state == HANDED_BACK and refused else result


def effects(repo: str, opts: argparse.Namespace) -> argparse.Namespace:
    """What [`queue_watch.run`] does to the world, bound to this repository."""
    common = git("rev-parse", "--git-common-dir", cwd=opts.root).stdout.strip() or ".git"
    return argparse.Namespace(
        pulls=lambda: quietly(
            "pr", "list", "--state", "open", "--limit", "200", "--json", queue_watch.LIST_FIELDS
        ),
        issue_labels=issue_labels,
        turn=lambda number: unready(drain([number], lambda n: take(repo, n, opts))[0]),
        head=lambda number: (quietly("pr", "view", str(number), "--json", "headRefOid") or {}).get("headRefOid"),
        clock=time.monotonic,
        sleep=time.sleep,
        say=say,
        memory=Path(opts.root, common, queue_watch.MEMORY),
        bots=BOTS,
        merged=MERGED,
        # A dry run previews every ready one; it hands nothing back for real.
        ends={GREEN, DRY} | ({HANDED_BACK} if opts.dry_run else set()),
        stops={STOPPED},
        skips={NOT_READY},
    )


def main(argv: list[str] | None = None) -> int:
    opts = parse(sys.argv[1:] if argv is None else argv)
    if not (opts.no_check or opts.dry_run):
        refused = preflight(target_dir(opts))
        if refused:
            say(f"refusing to start: {refused[0]}", *refused[1:])
            return 1
    repo = mergeable.gh("repo", "view", "--json", "nameWithOwner")["nameWithOwner"]
    if opts.watch:
        results, why, status = queue_watch.run(effects(repo, opts), opts)
    else:
        results = drain(ordered(opts.prs), lambda number: take(repo, number, opts))
    print()
    for line in summary(results):
        say(line)
    if opts.watch:
        say(f"watch ended: {why}")
        return status
    return 0 if all(state in (MERGED, GREEN, DRY) for _, state, _ in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
