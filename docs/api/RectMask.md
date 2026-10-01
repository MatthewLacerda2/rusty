## `RectMask`

Read and tune an entity's `RectMaskComponent` (#418) — Unity's `RectMask2D`. Every
graphic on the entity and below it is clipped to the axis-aligned bounds of its
rect in its canvas — the screen, or a world canvas's own plane (#619) — inset by
the padding; nested masks intersect. Add or remove the clip with
`Scene.AddComponent(id, "RectMask")` / `RemoveComponent` (adding also adds a
`RectTransform`). `GetPadding` returns zeros without one.

| Function | Signature | Returns |
|---|---|---|
| `RectMask.GetPadding` / `SetPadding` | `(id)` / `(id, l, b, r, t)` | inset in reference units (negative grows the clip) |
