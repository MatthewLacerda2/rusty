"""The workspace the round-gates tests plan against, and the repository's own recipes."""

from __future__ import annotations

import round_gates

CI = (round_gates.ROOT / round_gates.CI).read_text()
MAKEFILE = (round_gates.ROOT / "Makefile").read_text()
RECIPES = round_gates.recipes(MAKEFILE, CI)
MEMBERS = {"": "rusty", "crates/physics": "rusty-physics", "crates/core": "rusty-core",
           "crates/time": "rusty-time"}


def plan(*paths: str) -> round_gates.Plan:
    return round_gates.plan(list(paths), MEMBERS, CI)


def steps(*paths: str) -> list[tuple[str, list[str]]]:
    return round_gates.steps(plan(*paths), RECIPES)
