#!/usr/bin/env python3
"""`feature_split.py` — no dev-dependency turns on a feature the normal build lacks (#979).

Cargo resolves features for `cargo build` without the dev-dependencies. When a
dev-dependency switches on a feature the normal build leaves off, `cargo test`
compiles its own copy of that crate, of everything above it, and of rusty: proptest
did it to `getrandom`'s `std` feature and doubled ahash, hecs, wgpu and egui (#552).
`docs/testing.md` (*Build and test share one dependency graph*) made comparing the
two resolutions a manual step after every dependency bump; this is that step.

It runs the doc's two `cargo tree` commands and fails when a crate appears in both
with a feature the normal resolution lacks. A crate only in the dev resolution
(proptest and its tree) is test-only and passes. No build: about a second.
Exits 1 on a split, naming each crate and the features only tests turn on.
"""

from __future__ import annotations

import re
import subprocess
import sys

TREE = ["cargo", "tree", "--locked", "--features", "dev", "-f", "{p} [{f}]", "--prefix", "none"]
LINE = re.compile(r"^(\S+ v\S+)(?: \([^)]*\))? \[([^\]]*)\]")


def resolution(output: str) -> dict[str, set[str]]:
    """Each crate (`name vX.Y.Z`) and its enabled features, from `cargo tree` output."""
    crates: dict[str, set[str]] = {}
    for line in output.splitlines():
        match = LINE.match(line.strip())
        if match:
            features = {f for f in match.group(2).split(",") if f}
            crates.setdefault(match.group(1), set()).update(features)
    return crates


def splits(normal: str, with_dev: str) -> dict[str, set[str]]:
    """Crates in both resolutions, and the features only the dev one turns on."""
    build, test = resolution(normal), resolution(with_dev)
    return {
        crate: test[crate] - features
        for crate, features in sorted(build.items())
        if crate in test and test[crate] - features
    }


def main() -> int:
    run = lambda edges: subprocess.run(
        [*TREE, "-e", edges], check=True, capture_output=True, text=True).stdout
    found = splits(run("normal,build"), run("normal,build,dev"))
    for crate, extra in found.items():
        print(f"feature-split: {crate}: the test build adds {', '.join(sorted(extra))}")
    if found:
        print("  A dev-dependency turns on a feature the normal build lacks, so `cargo test`")
        print("  rebuilds the crate and everything above it. Name the feature on the normal")
        print("  dependency (see getrandom in Cargo.toml, #552) — docs/testing.md.")
        return 1
    print("feature-split: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
