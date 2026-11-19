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
  `cargo nextest` is `cargo-nextest`), `rustc` or `bench-scenes` process
  ([`BUSY`]), from the process table, before any build and again
  before each leg (a build started mid-run would skew the legs after it);
- **un-imported textures**: the images an import would encode now, which
  `bench-scenes --pending-imports` lists from the engine's own check
  (`asset::texture_import::pending`), so the refusal and the import never
  disagree. It needs a built engine, so it runs on `main`'s build before the
  PR's build and any leg, and again on the PR's build: a PR that changes the
  cache's stamp, a texture default or the encoder sees every cache as stale,
  uploads uncompressed, and its memory and load rows move for a reason that is
  not its code (`gpu_mb` 2176 against 1216 MB);
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
from pathlib import Path

from merging.watch import lock

LOCK = Path("out/bench/.timed-run.lock")
# Processes that skew a timed run: a build, or another bench.
BUSY = ("cargo", "rustc", "bench-scenes")


def hold(path: Path):
    """`(handle, None)` with the timed-run lock held while `handle` stays open,
    or `(None, refusal)` naming the run that holds it."""
    handle, holder = lock.hold(path)
    if handle is None:
        return None, f"another timed run holds {path} (PID {holder}): a timed benchmark runs alone"
    return handle, None


def flags_refusal(help_text: str, binary: Path, sets: tuple[str, ...]) -> tuple[str | None, str | None]:
    """`(refusal, pending_refusal)` for a binary whose `--help` is `help_text`.

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


def busy_refusal(found: list[str]) -> str | None:
    """The refusal for what [`busy`] found, if anything."""
    if not found:
        return None
    return f"a build or a bench runs on the machine ({', '.join(found)}): a timed benchmark runs alone"


def pending_refusal(code: int, said: str) -> str | None:
    """The refusal for `bench-scenes --pending-imports`' answer: exit 1 lists the images."""
    if code == 0:
        return None
    if code == 1:
        return f"the project has un-imported textures (make import PROJECT=<dir>):\n{said.strip()}"
    return f"bench-scenes --pending-imports failed (exit {code}): {said.strip()}"
