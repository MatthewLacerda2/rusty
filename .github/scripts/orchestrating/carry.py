"""The merge queue's state, carried in the lock's commit (#2667).

The queue keeps its memory in `.git/merge-queue/`: the hand-back memories, the
kept board, the waiting line, the cap, the mutation requests, the quiet watch's
`quiet.json`, the conflicts record. A cloud session's `.git` dies with it, and a
fresh fire that forgot every hand-back would take each handed-back pull request
again, merging `main` into unfixed branches. So each renewal and release saves
those files as the tree of the lock's commit ([`tree`]), and a side taking the
lock from another place restores them ([`restore`]) before it acts.

- **What is carried** ([`carried`]): every `*.json` and `*.jsonl` file directly
  in the directory, none starting with `.` (`merging/state.py`'s temporaries).
  The log (`queue.log`) and the locks are not: they are this machine's.
- **Restoring never deletes**: a file the tree lacks is left as it is, so the
  first take, from an empty lock, wipes nothing on this machine.
- **From another place only**: a local side taking back its own release keeps
  its files, which may be newer (a `make coder` after the release wrote the line).
  A cloud fire always restores: every fire is a fresh container.
- **The cap lives on the lock, not on a side** ([`SHARED`]): the coders' cap
  (`dispatch/cap.py`) is set on either machine whoever holds the lock, so
  `make coders --cap` pushes it into the lock's tree at once ([`share`]), a
  renewal or release keeps the tip's copy rather than the holder's
  ([`tree`]'s `base`), and every take restores it, from any place.
- **So does the schedule's id** ([`SHARED`], [`shared`]): `make orchestrator`
  stores the routine it created there, so `--stop` and the duplicate check
  find it by id from either machine, not only in a listing of the newest.
- **Each file is written whole** (a temporary, then a rename), as `merging/state.py` writes.
"""

from __future__ import annotations

import os
from pathlib import Path

from orchestrating import lease

SUFFIXES = (".json", ".jsonl")

#: The files whose copy on the lock is the one in force, whoever holds it: `dispatch/cap.py`'s
#: cap, and the id of the orchestrator's schedule (`orchestrating/schedule.py`, `FILE`).
SHARED = ("coders-cap.json", "orchestrator-schedule.json")


def carried(directory: Path) -> list[Path]:
    """The state files in `directory` that travel with the lock, by name."""
    if not directory.is_dir():
        return []
    return sorted(p for p in directory.iterdir()
                  if p.is_file() and p.suffix in SUFFIXES and not p.name.startswith("."))


def listing(git, tree_id: str) -> dict[str, str]:
    """`tree_id`'s state files, name to blob id."""
    status, out, err = git("ls-tree", tree_id)
    if status != 0:
        raise OSError(f"listing the state's tree: {err.strip()[:200]}")
    found = {}
    for row in out.splitlines():
        meta, _, name = row.partition("\t")
        kind, blob = meta.split()[1:3]
        if kind == "blob" and "/" not in name and name.endswith(SUFFIXES):
            found[name] = blob
    return found


def blob(git, text: str, name: str) -> str:
    status, made, err = git("hash-object", "-w", "--stdin", stdin=text)
    if status != 0:
        raise OSError(f"hashing {name}: {err.strip()[:200]}")
    return made.strip()


def tree(git, directory: Path, base: str | None = None) -> str:
    """A git tree of [`carried`]'s files, written to the object store; its id. The
    [`SHARED`] files come from the tree `base` (the lock's tip), never from `directory`."""
    blobs = {p.name: blob(git, p.read_text(), p.name) for p in carried(directory) if p.name not in SHARED}
    kept = listing(git, base) if base else {}
    return mktree(git, {**blobs, **{name: kept[name] for name in SHARED if name in kept}})


def mktree(git, blobs: dict[str, str]) -> str:
    rows = "".join(f"100644 blob {b}\t{name}\n" for name, b in sorted(blobs.items()))
    status, made, err = git("mktree", stdin=rows)
    if status != 0:
        raise OSError(f"making the state's tree: {err.strip()[:200]}")
    return made.strip()


def restore(git, tree_id: str, directory: Path, only: tuple[str, ...] | None = None) -> list[str]:
    """Write every file of `tree_id` (or of them, those named in `only`) into `directory`; their names."""
    written = []
    for name, found in listing(git, tree_id).items():
        if only is not None and name not in only:
            continue
        status, text, err = git("cat-file", "blob", found)
        if status != 0:
            raise OSError(f"reading {name}: {err.strip()[:200]}")
        directory.mkdir(parents=True, exist_ok=True)
        part = directory / f".{name}.part"
        part.write_text(text)
        os.replace(part, directory / name)
        written.append(name)
    return written


def share(git, name: str, text: str | None, why: str, now) -> None:
    """Put the [`SHARED`] file `name` (`None`: remove it) in the lock's tree, the lease left
    as it stands (`lease.amend`); `lease.Unreachable` when GitHub could not take it."""
    def changed(tip: lease.Lease) -> str:
        blobs = listing(git, tip.tree)
        if text is None:
            blobs.pop(name, None)
        else:
            blobs[name] = blob(git, text, name)
        return mktree(git, blobs)
    lease.amend(git, changed, why, now)


def shared(git, name: str) -> str | None:
    """The [`SHARED`] file `name` on the lock's tip now, or `None`; `lease.Unreachable` if unread."""
    found = listing(git, lease.read(git).tree).get(name)
    if found is None:
        return None
    status, text, err = git("cat-file", "blob", found)
    if status != 0:
        raise OSError(f"reading {name}: {err.strip()[:200]}")
    return text


def restores(before_place: str, me_place: str) -> bool:
    """Whether a take from a lease last written from `before_place` restores its state into `me_place`."""
    if not before_place:
        return False
    return me_place == "cloud" or before_place != me_place
