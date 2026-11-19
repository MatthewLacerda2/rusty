"""The three legs' reports as one table, the one a PR comment takes (#2381).

Per scene and row: `main`'s average, the PR's, the PR's change against `main`,
and the floor, the second `main` leg's change against the first. A row whose
change is no larger than its floor reads *not shown*: the two binaries differ
by no more than one binary differs from itself minutes later. Every row has its
own floor: the GPU rows and the counters repeat tightly where the CPU rows
drift, and the table shows that rather than assuming it.

A leg's report is `bench-scenes`' `<leg>/<project>-<hash>/<scene>.json`
(`dev::bench::scenes::report`); a leg holds one project's folder.
"""

from __future__ import annotations

import json
from pathlib import Path


def read(leg: Path, scene: str) -> dict | None:
    """The leg's report of `scene`, if it wrote one."""
    for path in sorted(leg.glob(f"*/{scene}.json")):
        try:
            return json.loads(path.read_text())
        except (OSError, ValueError):
            return None
    return None


def averages(report: dict | None) -> dict[str, float]:
    """Each row's average, in the report's order."""
    return {r["metric"]: r["avg"] for r in (report or {}).get("rows", [])}


def change(a: float, b: float) -> float | None:
    """`b` against `a` in %, `None` when `a` is 0 and `b` is not (no ratio)."""
    if a == 0:
        return 0.0 if b == 0 else None
    return (b - a) / abs(a) * 100.0


def pct(x: float | None) -> str:
    return "n/a" if x is None else f"{x:+.1f}%"


def num(x: float | None) -> str:
    if x is None:
        return "absent"
    return f"{x:.3f}".rstrip("0").rstrip(".") if abs(x) < 1000 else f"{x:.0f}"


def row(metric: str, main: float, pr: float | None, again: float | None) -> list[str]:
    """One table row: metric, main, PR, change, floor."""
    moved = None if pr is None else change(main, pr)
    floor = None if again is None else change(main, again)
    if pr is None:
        said = "absent on the PR"
    elif moved is None:
        said = f"{num(main)} → {num(pr)}"
    elif floor is None or abs(moved) <= abs(floor):
        said = f"not shown ({pct(moved)})"
    else:
        said = pct(moved)
    return [metric, num(main), num(pr), said, pct(floor) if floor is not None else "n/a"]


def scene_table(scene: str, legs: tuple[Path, Path, Path]) -> str:
    """The markdown table of `scene` over the three legs' folders (main, PR, main again)."""
    main, pr, again = (averages(read(leg, scene)) for leg in legs)
    if not main:
        return f"**{scene}**: the main leg wrote no report\n"
    lines = [f"**{scene}**", "", "| row | main | PR | change | floor (main again) |", "|---|---:|---:|---|---:|"]
    for metric, avg in main.items():
        lines.append("| " + " | ".join(row(metric, avg, pr.get(metric), again.get(metric))) + " |")
    return "\n".join(lines) + "\n"


def render(scenes: list[str], legs: tuple[Path, Path, Path], header: list[str]) -> str:
    """Every scene's table under the run's header lines."""
    parts = ["\n".join(header), ""] + [scene_table(s, legs) for s in scenes]
    return "\n".join(parts)


def scenes_of(leg: Path) -> list[str]:
    """The scenes a leg reported, by their name under `assets/scenes/`, sorted."""
    names = []
    for project in sorted(p for p in leg.iterdir() if p.is_dir()) if leg.is_dir() else []:
        for path in project.rglob("*.json"):
            names.append(path.relative_to(project).with_suffix("").as_posix())
    return sorted(set(names))
