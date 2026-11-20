"""What stops a timed run before it starts (#2381): a timed benchmark runs alone.

Two of #2298's measurements were wrong for a reason a check would have
refused: a baseline taken while two local agents built beside it (its CPU rows
spread 9%), and a run on a project whose textures were never imported (`gpu_mb`
2176 against 1216 once imported). So `make bench-ab` refuses:

- **another timed run**: an `fcntl.flock` on `out/bench/.timed-run.lock`
  (`merging/watch/lock.hold`, the merge queue's), held for the life of the
  process and dropped by the kernel however it dies, a power loss included, so a
  lock is never left behind for a reused PID to keep; the file names the
  holder's PID for the refusal;
- **a build or a bench on the machine**: a `cargo`, `cargo-<tool>` (a running
  `cargo nextest` is `cargo-nextest`), `rustc`, `bench-scenes` or `play` process
  ([`BUSY`]; `play` since `STRESS=1` benches the yard with it, #2450: another
  `make bench` beside a yard run skews both), from the process table, before any
  build and again before each leg and each yard run (a build started mid-run
  would skew the legs after it). What it finds is **waited out first**
  ([`settle`]: polled every [`POLL`] s for up to [`LIMIT`] s, one line when the
  wait starts and one when it ends), and refused only if still there: on
  2026-11-20 two runs were lost to a `cargo-fmt` gone within seconds (#2463),
  the merge queue's `make resolved` after merging `main` into a queued branch,
  one refused in its first second and one after 19 minutes of finished legs.
  A leg still never starts beside a build; the lock between two timed runs is
  not a transient tool and still refuses at once;
- **un-imported textures and models**: the images an import would encode now,
  and the models whose cooked file is missing or stale, which
  `bench-scenes --pending-imports` lists from the engine's own checks
  (`asset::texture_import::pending`, `asset::cook::pending`), so the refusal and
  the import never disagree (a model cooked at load anyway is noted in its scene's
  report). It needs a built engine, so it runs on `main`'s build before the
  PR's build and any leg, and again on the PR's build: a PR that changes the
  cache's stamp, a texture default or the encoder sees every cache as stale,
  uploads uncompressed, and its memory and load rows move for a reason that is
  not its code (`gpu_mb` 2176 against 1216 MB). Each refusal names the build
  that refused. One `.bctex` per source holds one stamp, so when one build finds
  the project imported and the other does not ([`textures`]), no import
  satisfies both and `make import` would only loop: the refusal says the builds
  disagree, and `FORCE=1` measures anyway, the table's header saying `gpu_mb` and
  the load rows are not comparable;
- **a binary that cannot do what is asked** ([`flags_refusal`]): each binary is
  asked its `--help` first. `main`'s has no `--pending-imports` (or `--set`)
  until the PR that adds them merges, and an unknown flag is read as the out
  folder, benching every scene into it: a binary without `--pending-imports`
  makes the texture check one that could not run (a refusal `FORCE=1` runs past,
  saying so), and one without `--set` asked for one refuses outright, `FORCE=1`
  or not, since no leg of it could run.

`FORCE=1` runs anyway, and the table says what was overridden.
"""

from __future__ import annotations

import os
import time
from pathlib import Path

from merging.watch import lock

LOCK = Path("out/bench/.timed-run.lock")
# Processes that skew a timed run: a build, or another bench.
BUSY = ("cargo", "rustc", "bench-scenes", "play")
# How often, and for how long, [`settle`] waits for what [`busy`] found to end (#2463):
# a formatter or a `make resolved` is seconds, a build minutes; past the limit it refuses.
POLL = 5.0
LIMIT = 600.0
# The clock and the sleep [`settle`] waits with, so a test drives a fake one.
CLOCK = time.monotonic
SLEEP = time.sleep


def hold(path: Path):
    """`(handle, None)` with the timed-run lock held while `handle` stays open,
    or `(None, refusal)` naming the run that holds it."""
    handle, holder = lock.hold(path)
    if handle is None:
        return None, f"another timed run holds {path} (PID {holder}): a timed benchmark runs alone"
    return handle, None


def flags_refusal(help_text: str, binary: Path, sets: tuple[str, ...]) -> tuple[str | None, str | None]:
    """`(refusal, cannot_check)` for a binary whose `--help` is `help_text`.

    The first stops the run whatever `FORCE` says (a `--set` it cannot take); the
    second is the texture check that cannot run, which `FORCE=1` runs past."""
    hard = None
    if sets and "--set" not in help_text:
        hard = (f"{binary} has no --set, so SET= cannot run on it: bench a PR whose legs both take it "
                f"(--set reaches main with the PR that adds it)")
    soft = None
    if "--pending-imports" not in help_text:
        soft = (f"the un-imported textures check could not run: {binary} has no --pending-imports "
                f"(main gains it with the PR that adds it); make import PROJECT=<dir> first, then FORCE=1")
    return hard, soft


def busy(ps: str, me: int) -> list[str]:
    """The processes in `ps` (`ps -A -o pid=,comm=`) that skew a timed run, as `name (PID n)`."""
    found = []
    for line in ps.splitlines():
        parts = line.split(None, 1)
        if len(parts) != 2 or not parts[0].isdigit() or int(parts[0]) == me:
            continue
        name = os.path.basename(parts[1].strip())
        if name in BUSY or name.startswith("cargo-"):
            found.append(f"{name} (PID {parts[0]})")
    return found


def settle(find, say) -> list[str]:
    """Wait while `find()` (a [`busy`] read) finds something, polling every [`POLL`] s for up
    to [`LIMIT`] s; what it still finds then (`[]` once the machine is quiet). `say(line)` is
    told once when a wait starts (what was found) and once when it ends (how long)."""
    found = find()
    if not found:
        return []
    say(f"bench-ab: waiting for {', '.join(found)} to end (polled every {POLL:.0f} s, "
        f"up to {LIMIT / 60:.0f} min): a timed benchmark runs alone")
    start = CLOCK()
    while found and CLOCK() - start < LIMIT:
        SLEEP(POLL)
        found = find()
    took = CLOCK() - start
    if found:
        say(f"bench-ab: waited {took:.0f} s and still found {', '.join(found)}")
    else:
        say(f"bench-ab: the machine is quiet after {took:.0f} s of waiting; going on")
    return found


def busy_refusal(found: list[str]) -> str | None:
    """The refusal for what [`busy`] found, if anything."""
    if not found:
        return None
    return f"a build or a bench runs on the machine ({', '.join(found)}): a timed benchmark runs alone"


def pending_answer(build: str, code: int, said: str) -> tuple[str, str | None]:
    """`bench-scenes --pending-imports`' answer (its stdout) as `(kind, text)`: `"clean"`,
    `"pending"` (exit 1, the images listed) or `"failed"`, the text naming `build`."""
    if code == 0:
        return "clean", None
    if code == 1:
        return "pending", f"{build}'s build finds un-imported textures or models in the project:\n{said.strip()}"
    return "failed", f"{build}'s bench-scenes --pending-imports failed (exit {code}): {said.strip()}"


DISAGREE = ("the two builds disagree on the import cache: {clean}'s finds the project imported, "
            "{pending}'s does not, so the PR changes a cache's stamp (a texture's, or the cooked models' "
            "COOKER_VERSION), a texture default or the encoder and no import satisfies both (one .bctex or "
            "cooked file per source holds one stamp). Measure with FORCE=1 and read "
            "gpu_mb and the load rows as not comparable; the table's header says so")
IMPORT = "make import PROJECT=<dir>, then run again"


def textures(main: tuple[str, str | None], pr: tuple[str, str | None], names=("main", "the PR")
             ) -> tuple[list[str], bool]:
    """The texture refusals from both builds' answers, and whether `gpu_mb` and the load rows compare.

    An answer is [`pending_answer`]'s, or `("cannot", text)` for a build that
    could not be asked ([`flags_refusal`])."""
    kinds = (main[0], pr[0])
    if sorted(kinds) == ["clean", "pending"]:
        pending = kinds.index("pending")
        listed = (main, pr)[pending][1].partition("\n")[2]
        said = DISAGREE.format(clean=names[1 - pending], pending=names[pending])
        return [f"{said}:\n{listed}" if listed else said], False
    refusals = []
    for kind, text in (main, pr):
        if kind == "pending":
            refusals.append(f"{text}\n({IMPORT})")
        elif kind != "clean":
            refusals.append(text)
    return refusals, True
