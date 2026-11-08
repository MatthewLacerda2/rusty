"""Two runs the queue lost a round to on 2026-11-08 (#1932), trimmed from the jobs API.

Each job keeps its name, conclusion and steps (`number`, `name`, `conclusion`);
the green jobs beside the red ones are dropped. [`MACOS_NEXTEST`] is run
42466064410's `build-test-cross (macos-26)`, failed in `Install cargo-nextest`
(a 504 from the release host); [`DENY_IMAGE`] is run 42468896745 attempt 1's
`deny`, failed building the cargo-deny action's image.
"""

from __future__ import annotations


def steps(*named: tuple[str, str]) -> list[dict]:
    return [{"number": n, "name": name, "conclusion": c} for n, (name, c) in enumerate(named, 1)]


MACOS_NEXTEST = {
    "name": "build-test-cross (macos-26)",
    "conclusion": "failure",
    "steps": steps(
        ("Set up job", "success"),
        ("Run actions/checkout@v7", "success"),
        ("Toolchain (from rust-toolchain.toml)", "success"),
        ("Cache cargo registry + target", "success"),
        ("Install cargo-nextest", "failure"),
        ("Test (engine, default)", "skipped"),
        ("Test (engine, dev features)", "skipped"),
        ("Doctests (both feature sets)", "skipped"),
        ("Test summary", "success"),
        ("Test (lint xtask)", "skipped"),
        ("Post Cache cargo registry + target", "skipped"),
        ("Post Run actions/checkout@v7", "success"),
        ("Complete job", "success"),
    ),
}

DENY_IMAGE = {
    "name": "deny",
    "conclusion": "failure",
    "steps": steps(
        ("Set up job", "success"),
        ("Build EmbarkStudios/cargo-deny-action@v2", "failure"),
        ("Run actions/checkout@v7", "skipped"),
        ("Run EmbarkStudios/cargo-deny-action@v2", "skipped"),
        ("Complete job", "success"),
    ),
}

GATE_RED = {
    "name": "ci-gate",
    "conclusion": "failure",
    "steps": steps(
        ("Set up job", "success"),
        ("Gate on the correctness jobs (success or skipped passes)", "failure"),
        ("Complete job", "success"),
    ),
}


def failed_at(job: dict, name: str) -> dict:
    """`job` with its setup failure moved onto step `name` (and nothing else failed)."""
    moved = [{**s, "conclusion": "failure" if s["name"] == name else "success"} for s in job["steps"]]
    return {**job, "steps": moved}
