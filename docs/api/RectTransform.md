## `RectTransform`

Read and tune an entity's `RectTransformComponent` (#417) — 2D placement inside
the parent's rect, with Unity's exact semantics. Every value is an `x, y` pair in
the canvas's reference units, **y-up, origin bottom-left**. The anchors mark a
region of the parent rect (a point when min == max, a stretch otherwise);
`SizeDelta` is the size *minus* that region; `AnchoredPosition` is the pivot's
offset from the anchors lerped by the pivot. The entity's `Transform` keeps
**rotation and scale** (applied around the pivot); its position is ignored for a
rect-laid-out entity. Getters return `0, 0` when the entity has no RectTransform.
In `Debug.Snapshot` the component appears as `rect_transform`.

| Function | Signature | Returns |
|---|---|---|
| `RectTransform.GetAnchorMin` / `SetAnchorMin` | `(id)` / `(id, x, y)` | `x, y` (clamped to 0..1; pushes `AnchorMax` up to stay ≥) |
| `RectTransform.GetAnchorMax` / `SetAnchorMax` | `(id)` / `(id, x, y)` | `x, y` (clamped to 0..1; pulls `AnchorMin` down to stay ≤) |
| `RectTransform.GetPivot` / `SetPivot` | `(id)` / `(id, x, y)` | `x, y` (fraction of the element's own rect) |
| `RectTransform.GetAnchoredPosition` / `SetAnchoredPosition` | `(id)` / `(id, x, y)` | `x, y` |
| `RectTransform.GetSizeDelta` / `SetSizeDelta` | `(id)` / `(id, x, y)` | `x, y` |
| `RectTransform.SetAnchorPreset` | `(id, x, y, setPivot?, setPosition?)` | — the editor's anchor-preset grid: `x` is `"left"`/`"center"`/`"right"`/`"stretch"`, `y` is `"bottom"`/`"middle"`/`"top"`/`"stretch"`. Re-anchors **without moving** the element (position and size delta re-derived against the parent's rect); `setPivot` also moves the pivot to the preset point (centre on a stretched axis); `setPosition` also snaps the element onto its anchors (a stretched axis fills the parent). An unknown name is an error |
| `RectTransform.SetWorldAnchor` | `(id, target, ox, oy, oz)` | — makes the element a **marker** pinned to entity `target` + offset (metres); `target = nil` pins it to the world point `(ox, oy, oz)` |
| `RectTransform.ClearWorldAnchor` | `(id)` | — back to ordinary anchors |
| `RectTransform.GetWorldAnchor` | `(id)` | `{ target, offset = {x, y, z}, clamp_to_screen_edge, edge_padding, rotate_toward_target, hide_when_behind }`, or `nil` for a non-marker |
| `RectTransform.SetWorldAnchorClamp` | `(id, clamp, padding)` | — keep it on screen, `padding` reference units inside the edge |
| `RectTransform.SetWorldAnchorRotate` | `(id, on)` | — while clamped, turn its up toward the target (an off-screen arrow) |
| `RectTransform.SetWorldAnchorHideWhenBehind` | `(id, on)` | — hide it (and its children) while the target is behind the camera (default on) |

**Markers (#429).** A world anchor turns a screen-space element into a marker —
an enemy health bar, a waypoint, a damage number. Every layout pass projects the
target (its world position + offset) through the active camera and puts the
element's **pivot** there; its size still comes from its anchors and `SizeDelta`.
Off screen: behind the camera with `hide_when_behind` → hidden; with
`clamp_to_screen_edge` → pinned to the padded screen edge toward the target
(turned toward it with `rotate_toward_target`); otherwise at the raw projection.
A destroyed target hides the marker. Markers only apply on `ScreenSpaceOverlay` /
`ScreenSpaceCamera` canvases, and need a camera (`UI.GetRect` and the per-tick
layout use the active one). The option setters do nothing to a non-marker; saving
a prefab keeps a target inside it and drops one outside.

```lua
-- A health bar floating 2 m above an enemy, clamped to the screen edge.
RectTransform.SetWorldAnchor(bar, enemy, 0, 2, 0)
RectTransform.SetWorldAnchorClamp(bar, true, 24)
```
