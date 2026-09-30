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
