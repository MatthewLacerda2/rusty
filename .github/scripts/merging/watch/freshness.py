"""Whether the watch still runs the code on `main` (#967).

Python reads the queue's code once, at launch, so a merge that changes it
leaves the running watch deciding merges with the old code: on 2026-10-04
#751 and #753 both landed under a watch still running the code they replaced.
So the watch notes the blob of every file it runs at launch ([`launched`]),
taken from the import graph so a module split out later (#926) is included,
and before each take compares them with `origin/main` ([`changed`]). On a
difference it exits with [`ending.RELAUNCH`], and whoever started it updates the
checkout and starts it again.
"""

from __future__ import annotations

import sys
from pathlib import Path


# The package whose loaded modules are the watch's code, beside the entry script.
PACKAGE = "merging"


def running(modules=None, entry: str | None = None) -> list[Path]:
    """The files of the queue's code this process loaded: every module of
    [`PACKAGE`] in `modules` (default `sys.modules`), and the entry script."""
    modules = sys.modules if modules is None else modules
    files = {
        Path(m.__file__).resolve() for name, m in list(modules.items())
        if (name == PACKAGE or name.startswith(PACKAGE + ".")) and getattr(m, "__file__", None)
    }
    main = entry if entry is not None else getattr(modules.get("__main__"), "__file__", None)
    if main and Path(main).name.endswith(".py"):
        files.add(Path(main).resolve())
    return sorted(files)


def launched(files: list[Path], git) -> dict[str, str]:
    """Each file's repo path and the blob of its content now, as git would
    store it. `git(*args, cwd=...)` returns `(status, stdout)`; a file git
    cannot place in a checkout is left out (nothing to compare it with)."""
    blobs = {}
    for file in files:
        status, top = git("rev-parse", "--show-toplevel", cwd=str(file.parent))
        status2, blob = git("hash-object", str(file), cwd=str(file.parent))
        if status == 0 and status2 == 0 and top.strip():
            blobs[file.relative_to(Path(top.strip()).resolve()).as_posix()] = blob.strip()
    return blobs


def changed(blobs: dict[str, str], git) -> list[str]:
    """The paths whose blob on `origin/main` is not the one launched, after
    fetching `main`. A path gone from `main` has changed too. When git
    cannot read `main` at all it fails open: nothing has changed."""
    if not blobs:
        return []
    git("fetch", "--quiet", "origin", "main")
    if git("rev-parse", "--verify", "--quiet", "origin/main^{commit}")[0] != 0:
        return []
    moved = []
    for path, blob in sorted(blobs.items()):
        status, now = git("rev-parse", "--verify", "--quiet", f"origin/main:{path}")
        if status != 0 or now.strip() != blob:
            moved.append(path)
    return moved
