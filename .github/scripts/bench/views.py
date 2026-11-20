"""The editor round's Scene view in each leg (#2469): the line a scene's section opens
with when `main`'s and the PR's views were drawn at other sizes.

The Scene view is what the editor's panels leave of the window, so a PR that changes a
panel's width changes the pixels every per-pixel pass shades: `editor.gpu_ms` and its
`editor.gpu.<pass>` parts all move alike with no change to the renderer. Measuring the
Hierarchy's visible-rows change, `editor.gpu_ms` read +6.3% on one level and every pass
rose with it, and only the PR's description said the view had grown. The editor round
reports the view's size (`editor.view_w`, `editor.view_h`,
`dev::bench::scenes::editor::VIEW_ROWS`), and the section says when the legs differ, in
the words the printed report uses (`SceneReport::view_moved`). A report from before
those rows reads as absent: no line.
"""

from __future__ import annotations

ROWS = ("editor.view_w", "editor.view_h")


def size(report: dict | None) -> tuple[float, float] | None:
    """The Scene view's width and height in the report, `None` when it lacks either."""
    found = {r["metric"]: r["avg"] for r in (report or {}).get("rows", []) if r["metric"] in ROWS}
    if len(found) < len(ROWS):
        return None
    return found[ROWS[0]], found[ROWS[1]]


def moved(main: dict | None, pr: dict | None) -> list[str]:
    """The line heading a scene's section when the PR's Scene view is another size
    than `main`'s, and a blank after it; nothing when they agree or either lacks it."""
    before, now = size(main), size(pr)
    if before is None or now is None or before == now:
        return []
    return [f"- **editor round: the Scene view is {px(now)} against {px(before)} on main**: "
            "the editor's GPU rows are not comparable pixel for pixel", ""]


def px(view: tuple[float, float]) -> str:
    """A view's size as the report prints it: `1416x963`."""
    return f"{view[0]:.0f}x{view[1]:.0f}"
