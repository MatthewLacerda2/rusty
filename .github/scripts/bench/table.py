"""The three legs' reports as one table, the one a PR comment takes (#2381).

Per scene and row: `main`'s average, the PR's, the PR's change against `main`,
and the floor, the second `main` leg's change against the first. A row whose
change is no larger than its floor reads *inside the floor*: the two binaries
differ by no more than one binary differs from itself minutes later. Every row has its
own floor: the GPU rows and the counters repeat tightly where the CPU rows
drift, and the table shows that rather than assuming it. A row the second
`main` leg lacks has no floor: its change is shown, the floor marked *unknown*,
never hidden behind *inside the floor*.

The rows are the union of `main`'s and the PR's, `main`'s order first: a row
only the PR reports (a new counter, a new `render.<stage>`) is the PR's own
measurement and reads *absent on main*, never dropped.

Each scene's table opens with a lead ([`lead`]): the rows whose change is
outside the floor and at least [`least`] in size, largest change first, then
those of that size with no floor to judge them by (*floor unknown*) and every
row only the PR has (*absent on main*), so the lead never says nothing moved
above a table that shows a move it cannot judge. The
first real use's table ran about 120 rows a scene, most inside the floor or
zero, and the row a PR is about had to be searched for (#2323). The size is
absolute because a ratio is not: a row at 0.001 ms moving 20% is nothing.

The load rows' counts (`load.count.<what>`: model files the scene uses, sources
converted, textures compressed and decoded, MB uploaded) say what each leg's
load did, and `models_converted` tells a cached load from a cold import; where they
differ between legs, [`count_changes`] says so for the run's header, as the
texture refusals do, since the load rows then compare different loads.

Above the lead, a scene's notes ([`notes`]): the warnings `bench-scenes` wrote
into each leg's report (a bake the scene lacks, textures drawn uncompressed,
console errors), each said once, and those not every leg wrote marked with the legs
that did. A note is why a table's absolute numbers may not be the game's (#2420),
so it heads the section the reader reads, not only the leg's own output.

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


LEGS = ("main", "PR", "main again")


def warnings(leg: Path, report: dict | None) -> list[str]:
    """The report's warnings, the leg's own folder spelled `<leg>` so one note
    (the console kept beside the report) reads the same in every leg."""
    spelled = sorted({str(leg), str(leg.resolve())}, key=len, reverse=True)
    out = []
    for said in (report or {}).get("warnings", []):
        for path in spelled:
            said = said.replace(path, "<leg>")
        out.append(said)
    return out


def notes(scene: str, legs: tuple[Path, Path, Path]) -> list[str]:
    """The lines heading `scene`'s section: each leg's warning once, in the order
    the legs first wrote them, one not every leg wrote marked with those that did."""
    said = [warnings(leg, read(leg, scene)) for leg in legs]
    seen = list(dict.fromkeys(w for leg in said for w in leg))
    if not seen:
        return []
    lines = []
    for w in seen:
        wrote = [name for name, leg in zip(LEGS, said) if w in leg]
        only = "" if len(wrote) == len(legs) else f" (only {', '.join(wrote)}: the legs disagree)"
        lines.append(f"- {w}{only}")
    return ["**Warnings** (`bench-scenes`' notes; the absolute numbers may not be the game's):"] + lines + [""]


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


def row(metric: str, main: float | None, pr: float | None, again: float | None) -> list[str]:
    """One table row: metric, main, PR, change, floor."""
    if main is None:
        return [metric, num(main), num(pr), "absent on main", "n/a"]
    moved = None if pr is None else change(main, pr)
    floor = None if again is None else change(main, again)
    if pr is None:
        said = "absent on the PR"
    elif moved is None:
        said = f"{num(main)} → {num(pr)}"
    elif again is None:
        said = f"{pct(moved)} (floor unknown)"
    elif floor is None or abs(moved) <= abs(floor):
        said = f"inside the floor ({pct(moved)})"
    else:
        said = pct(moved)
    if again is None:
        shown = "unknown: absent on main again"
    else:
        shown = pct(floor) if floor is not None else f"{num(main)} → {num(again)}"
    return [metric, num(main), num(pr), said, shown]


def least(metric: str) -> float:
    """The smallest move of `metric` the lead lists: 0.05 for a ms row (0.3% of a 60 Hz
    frame) and a count (which moves by whole units, so any change of one passes), 50
    for a µs row, 1 for MB and for a percentage."""
    if metric.endswith(("_mb", "_pct")):
        return 1.0
    return 50.0 if metric.endswith("_us") else 0.05


LEAST = "a move of at least 0.05 (ms, or a count), 50 for a µs row, 1 for an MB or % row"


def outside(a: float, b: float, again: float) -> float | None:
    """How far `b` moved from `a` (its % change, `inf` from zero) when the move is
    outside the floor `again` set; `None` inside it."""
    moved, floor = change(a, b), change(a, again)
    if moved is None or floor is None:  # from zero: the absolute moves compare
        return float("inf") if abs(b - a) > abs(again - a) else None
    return abs(moved) if abs(moved) > abs(floor) else None


def lead(main: dict, pr: dict, again: dict) -> list[str]:
    """The lines above a scene's table: its rows outside the floor, largest change
    first, then the rows that have no floor (the second `main` leg lacks them) and
    the rows only the PR has, each marked so, as the table marks them."""
    found, unfloored = [], []
    for metric, a in main.items():
        b, f = pr.get(metric), again.get(metric)
        if b is None or abs(b - a) < least(metric):
            continue
        moved = change(a, b)
        said = f"{num(a)} → {num(b)}" + ("" if moved is None else f", {pct(moved)}")
        if f is None:
            unfloored.append(f"- `{metric}` {said} (floor unknown)")
            continue
        size = outside(a, b, f)
        if size is not None:
            floor = change(a, f)
            found.append((size, f"- `{metric}` {said} (floor {pct(floor) if floor is not None else num(f)})"))
    new = [f"- `{m}` {num(b)} (absent on main)" for m, b in pr.items() if m not in main]
    found.sort(key=lambda x: -x[0])
    lines = [line for _, line in found] + unfloored + new
    if not lines:
        return [f"Nothing moved outside the floor by at least the lead's size ({LEAST})."]
    return [f"Outside the floor, largest change first, then rows with no floor ({LEAST}):"] + lines


def scene_table(scene: str, legs: tuple[Path, Path, Path]) -> str:
    """The markdown table of `scene` over the three legs' folders (main, PR, main again)."""
    main, pr, again = (averages(read(leg, scene)) for leg in legs)
    if not main and not pr:
        return f"**{scene}**: neither the main leg nor the PR's wrote a report\n"
    lines = [f"**{scene}**", ""] + notes(scene, legs) + lead(main, pr, again) + ["", "| row | main | PR | change | floor (main again) |",
                                                            "|---|---:|---:|---|---:|"]
    metrics = list(main) + [m for m in pr if m not in main]
    for metric in metrics:
        lines.append("| " + " | ".join(row(metric, main.get(metric), pr.get(metric), again.get(metric))) + " |")
    return "\n".join(lines) + "\n"


def render(scenes: list[str], legs: tuple[Path, Path, Path], header: list[str]) -> str:
    """Every scene's table under the run's header lines."""
    parts = ["\n".join(header), ""] + [scene_table(s, legs) for s in scenes]
    return "\n".join(parts)


def scenes_of_all(legs) -> list[str]:
    """The scenes any of `legs` reported, sorted: a scene only the PR reports still gets its table."""
    names = []
    for leg in legs:
        for project in sorted(p for p in leg.iterdir() if p.is_dir()) if leg.is_dir() else []:
            for path in project.rglob("*.json"):
                names.append(path.relative_to(project).with_suffix("").as_posix())
    return sorted(set(names))


def count_changes(scenes: list[str], legs) -> list[str]:
    """A header line per scene whose `load.count.<what>` rows differ between legs."""
    out = []
    for scene in scenes:
        reports = [averages(read(leg, scene)) for leg in legs]
        said = []
        for metric in (m for m in reports[0] if m.startswith("load.count.")):
            values = [r.get(metric) for r in reports]
            if any(v is not None and v != values[0] for v in values[1:]):
                legs_said = ", ".join(f"{n} {num(v)}" for n, v in zip(("main", "PR", "main again"), values))
                said.append(f"{metric} {legs_said}")
        if said:
            out.append(f"- **the load's counts differ between legs** in `{scene}` ({'; '.join(said)}): "
                       "its load rows compare a different load (the import cache), not only different code")
    return out
