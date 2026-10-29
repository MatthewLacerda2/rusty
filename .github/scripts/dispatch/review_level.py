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
"""

from __future__ import annotations

#: Why a path calls for `high`, then the paths. A miss there is a broken save,
#: a desynced harness, an escape from the sandbox, or a queue that merges the
#: wrong thing for every PR behind it.
TABLE: dict[str, tuple[str, ...]] = {
    "stored format": (
        "src/scene/serialize.rs",  # World <-> SceneData, the scene document
        "src/scene/io/",  # save / load, the on-disk text, the default scene's seeding
        "src/scene/prefab/",  # PrefabData, instances and overrides
        "src/core/storage.rs",  # Storage, the PlayerPrefs save file
        "src/asset/sidecar/",  # an asset's import settings beside it
        "src/asset/convert/",  # the import cache and its fingerprints
    ),
    "determinism": (
        "src/core/random.rs",  # the seeded Random resource
        "src/core/collections.rs",  # the deterministic maps the sim must use
        "src/api/random.rs",  # Random, the script's view of that stream
        "clippy.toml",  # the disallowed clocks, RNGs and std hash maps
        "tools/lint/src/determinism/",  # `make determinism`, the guard's opt-outs
    ),
    "scripting sandbox": (
        "src/scripting/sandbox/",  # the gameplay VM's stdlib: no os/io, require's reach
    ),
    "merge path": (
        ".github/scripts/merge-queue.py",
        ".github/scripts/merging/",
        ".github/scripts/mergeable.py",
        ".github/scripts/queue-daemon.py",  # the queue's supervisor
        ".github/scripts/ghrest.py",  # how the verdict reads a pull request
        ".github/workflows/",
    ),
}

HIGH, MEDIUM = "high", "medium"


def reasons_for(path: str) -> list[str]:
    """Each responsibility of [`TABLE`] that `path` falls under."""
    return [why for why, entries in TABLE.items()
            if any(path == entry or (entry.endswith("/") and path.startswith(entry)) for entry in entries)]


def level(paths: list[str]) -> tuple[str, list[tuple[str, str]]]:
    """The level, and every `(reason, path)` that decided it (empty for medium)."""
    decided = [(why, path) for path in sorted(set(paths)) for why in reasons_for(path)]
    return (HIGH if decided else MEDIUM), decided


def changed_paths(files: list[dict]) -> list[str]:
    """Every path a PR's REST `files` touch: a rename counts both of its names."""
    paths = []
    for entry in files:
        paths.append(entry.get("filename", ""))
        if entry.get("previous_filename"):
            paths.append(entry["previous_filename"])
    return [p for p in paths if p]
