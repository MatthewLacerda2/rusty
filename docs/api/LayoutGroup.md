## `LayoutGroup`

Read and tune an entity's `LayoutGroupComponent` (#421) — Unity's
`HorizontalLayoutGroup`, `VerticalLayoutGroup` and `GridLayoutGroup` in one
component. The group places each **layout child** (active, carrying a
`RectTransform`, not `LayoutElement.IgnoreLayout`) inside its own rect, overriding
the child's anchors: a row (`"Horizontal"`), a column (`"Vertical"`) or a grid of
equal cells (`"Grid"`). Children are sized from their min / preferred / flexible
sizes — a `Text`'s measured block, an `Image`'s native texture size, a nested
group, or a `LayoutElement` override. The layout runs every tick in the sim;
read where a child landed with `UI.GetRect`. The child RectTransforms are never
rewritten. Adding one also adds a `RectTransform`. Getters return a neutral
default (zeros, `false`, `""`) without a group; setters are then no-ops. Enum
values are names, case-insensitive; an unknown name is ignored. The model is in
[`docs/ui.md`](../ui.md#layout-groups).

| Function | Signature | Returns |
|---|---|---|
| `LayoutGroup.GetKind` / `SetKind` | `(id)` / `(id, name)` | `"Horizontal"`, `"Vertical"` or `"Grid"` |
| `LayoutGroup.GetPadding` / `SetPadding` | `(id)` / `(id, l, b, r, t)` | inset of the children from the rect, reference units |
| `LayoutGroup.GetSpacing` / `SetSpacing` | `(id)` / `(id, x, y)` | gap between columns (`x`, a row's gap) and rows (`y`, a column's gap) |
| `LayoutGroup.GetChildAlignment` / `SetChildAlignment` | `(id)` / `(id, name)` | where the children sit when they do not fill the rect — a `Text` alignment name (`"TopLeft"` … `"BottomRight"`) |
| `LayoutGroup.GetControlChildSize` / `SetControlChildSize` | `(id)` / `(id, width, height)` | row / column: whether the group sizes its children (else each keeps its `size_delta`) |
| `LayoutGroup.GetChildForceExpand` / `SetChildForceExpand` | `(id)` / `(id, width, height)` | row / column: whether every child shares the spare space (flexible ≥ 1) |
| `LayoutGroup.GetCellSize` / `SetCellSize` | `(id)` / `(id, x, y)` | grid: every cell's size (≥ 0) |
| `LayoutGroup.GetConstraint` / `SetConstraint` | `(id)` / `(id, name)` | grid: `"Flexible"` (as many columns as fit), `"FixedColumnCount"` or `"FixedRowCount"` |
| `LayoutGroup.GetConstraintCount` / `SetConstraintCount` | `(id)` / `(id, n)` | grid: the fixed column / row count (≥ 1) |
| `LayoutGroup.GetStartCorner` / `SetStartCorner` | `(id)` / `(id, name)` | grid: `"UpperLeft"`, `"UpperRight"`, `"LowerLeft"` or `"LowerRight"` |
| `LayoutGroup.GetStartVertical` / `SetStartVertical` | `(id)` / `(id, bool)` | grid: fill columns first instead of rows |
