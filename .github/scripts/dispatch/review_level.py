"""`high` or `medium` for an independent review, from the paths a PR touches (#1569).

`independent-review`, *Level*: a review runs at `high` when a miss would cost
every PR behind it or every game after it, and at `medium` otherwise. The rule
is about what the diff touches, which is mostly a matter of paths, so it is
applied here, the same way every time, and the reviewer spends no judgement on
it. The reviewer may still raise `medium` to `high` and say why: a path table
cannot see a stored format changed through a component's serde fields, say.

[`TABLE`] is the rule. An entry ending in `/` is a directory and covers what is
under it; any other entry is one file. Add a row when a new module takes on one
of these responsibilities, in the same PR.

**A stored format declares itself where it lives** (#1966). The table missed a
new on-disk format three times in one batch (#1946's hull sidecars, #1942's
`.animgraph`, PR #1962's `.occlusion`): each was a module it had not heard of.
So a module that keeps a file says `//! stored-format: <the file>` in its top
doc comment, `tools/lint`'s `stored-format` rule fails a serde type written to
disk without one, and [`covering`] reads the markers: one in `mod.rs` covers its
directory and everything under it, one in `x.rs` covers that file and `x/`. The
markers come from `origin/main` (`git grep`, one local call, [`tree_markers`])
plus any marker line a PR's own patches add or remove ([`patch_markers`]), so a
PR that brings a new format reads `high` too. Only what a marker cannot express
(a fingerprint file, WGSL bindings, a path outside the modules) stays a hand row.

**A module reads the same before and after its move into a crate** (#2139): a
path, a table entry and a marker are each compared at its module path
(`modules.module_path`), so `crates/core/src/random.rs` is the row
`src/core/random.rs` was, and a crate's `lib.rs` marker covers the crate as a
`mod.rs` one covers its directory.
"""

from __future__ import annotations

import subprocess
import sys

import modules

#: Why a path calls for `high`, then the paths. A miss there is a broken save,
#: a desynced harness, an escape from the sandbox, or a queue that merges the
#: wrong thing for every PR behind it.
TABLE: dict[str, tuple[str, ...]] = {
    "stored format": (
        # every serde change to the scene/prefab document moves it, wherever the
        # field lives (a component's own module): #1587 changed the format from
        # `src/components/mesh/` and read `medium`
        "tests/scene_format/scene.fingerprint",
        # surface-shader variants baked to `assets/shaders/` import these symbols by
        # name, so a rename strands every baked file (#1638; #1628 read `medium`)
        "engine/shaders/forward/bindings.wgsl",
    ),
    "determinism": (
        "crates/core/src/random.rs",  # the seeded Random resource
        "crates/core/src/collections.rs",  # the deterministic maps the sim must use
        "crates/script/src/api/random.rs",  # Random, the script's view of that stream
        "clippy.toml",  # the disallowed clocks, RNGs and std hash maps
        "tools/lint/src/determinism/",  # `make determinism`, the guard's opt-outs
        # which modules are sim and which crates each may name: a new crate in a sim
        # row is new code the sim runs (#1634 moved `ui`'s text layout onto two
        # crates and read `medium`)
        "tools/lint/src/layers/table.rs",
        # record and replay: a replay that diverges is a desynced harness (#1636's
        # seek/restore read `medium`)
        "crates/dev/src/demo/",
        # which `Debug.Stats` keys are wall-clock and stay out of the harness's
        # `results.json`: a timing key let through makes two runs of one seed differ
        # (#1872 found `render_extract_us` leaking, and read `medium`)
        "crates/core/src/frame_stats.rs",
    ),
    "scripting sandbox": (
        "crates/script/src/scripting/sandbox/",  # the gameplay VM's stdlib: no os/io, require's reach
    ),
    "merge path": (
        ".github/scripts/merge-queue.py",
        ".github/scripts/merging/",
        ".github/scripts/mergeable.py",
        ".github/scripts/queue-daemon.py",  # the queue's supervisor
        ".github/scripts/ghrest.py",  # how the verdict reads a pull request
        ".github/workflows/",
        ".github/actions/",  # the workflows' shared steps (#1973)
    ),
}

HIGH, MEDIUM = "high", "medium"
MARKER = "//! stored-format:"
#: `git grep -E` for a marker line, indented or not.
PATTERN = r"^[[:space:]]*//! stored-format:"


def covering(path: str, marked: dict[str, str]) -> str | None:
    """The text of the nearest marker covering `path`: the file's own, then each
    enclosing directory's `mod.rs` or `<dir>.rs`, as `tools/lint`'s rule reads them.
    Both sides are read at their module paths."""
    path = modules.module_path(path)
    marked = {modules.module_path(p): why for p, why in marked.items()}
    if path in marked:
        return marked[path]
    parent = path
    while "/" in parent:
        parent = parent.rsplit("/", 1)[0]
        for owner in (f"{parent}/mod.rs", f"{parent}.rs"):
            if owner in marked:
                return marked[owner]
    return None


def parse_marker(line: str) -> str | None:
    """The text of a marker line, `None` for any other line."""
    line = line.strip()
    return line[len(MARKER):].strip() if line.startswith(MARKER) else None


def tree_markers(rev: str | None = "origin/main", cwd=None) -> dict[str, str]:
    """`path -> text` for every marker under `src/` and `crates/` at `rev` (the working tree when
    `None`, or when `rev` cannot be read). A marker may be indented, as the lint's
    `marker()` and [`parse_marker`] read it. Says on stderr when it falls back or
    finds nothing, since either makes every marked path read `medium`."""
    tree = [rev] if rev else []
    done = subprocess.run(["git", "grep", "-I", "-E", "--full-name", "-e", PATTERN, *tree, "--", "src", modules.CRATES],
                          capture_output=True, text=True, check=False, cwd=cwd)
    if done.returncode not in (0, 1):
        print(f"review-level: cannot read {rev or 'the working tree'}'s markers"
              + ("; reading the working tree" if rev else ""), file=sys.stderr)
        return tree_markers(None, cwd) if rev else {}
    marked = {}
    for hit in done.stdout.splitlines():
        if rev and hit.startswith(f"{rev}:"):
            hit = hit[len(rev) + 1:]
        path, _, text = hit.partition(":")
        if (why := parse_marker(text)) is not None:
            marked[path] = why
    if not marked:
        print(f"review-level: no stored-format marker in {rev or 'the working tree'}", file=sys.stderr)
    return marked


def patch_markers(files: list[dict]) -> dict[str, str]:
    """`path -> text` for every marker line a PR's patch adds or removes."""
    marked = {}
    for entry in files:
        for line in (entry.get("patch") or "").splitlines():
            if line[:1] in "+-" and (why := parse_marker(line[1:])) is not None:
                marked[entry.get("filename", "")] = why
    return marked


def reasons_for(path: str, marked: dict[str, str] | None = None) -> list[str]:
    """Each responsibility of [`TABLE`] that `path` falls under, then the stored
    format a marker covering it names."""
    at = modules.module_path(path)
    reasons = [why for why, entries in TABLE.items()
               if any(at == (row := modules.module_path(entry)) or (row.endswith("/") and at.startswith(row))
                      for entry in entries)]
    if (text := covering(path, marked or {})) is not None:
        reasons.append(f"stored format ({text})")
    return reasons


def level(paths: list[str], marked: dict[str, str] | None = None) -> tuple[str, list[tuple[str, str]]]:
    """The level, and every `(reason, path)` that decided it (empty for medium)."""
    decided = [(why, path) for path in sorted(set(paths)) for why in reasons_for(path, marked)]
    return (HIGH if decided else MEDIUM), decided


def changed_paths(files: list[dict]) -> list[str]:
    """Every path a PR's REST `files` touch: a rename counts both of its names."""
    paths = []
    for entry in files:
        paths.append(entry.get("filename", ""))
        if entry.get("previous_filename"):
            paths.append(entry["previous_filename"])
    return [p for p in paths if p]
