## `Mask`

Read and tune an entity's `MaskComponent` (#428) — Unity's `Mask`. Every graphic
*below* the entity is clipped to the entity's own graphic: its `Image`'s texture
alpha times its colour alpha, else its [`Shape`](Shape.md)'s coverage, else its
rect — so an `Ellipse` Shape or a circle sprite gives a round minimap and a
soft-edged sprite a feathered edge.
Nested masks multiply, to any depth, and a `RectMask` above still applies. The
pointer cannot hit a child outside the mask's rect. Works on screen and world
canvases alike. Add or remove one with `Scene.AddComponent(id, "Mask")` /
`RemoveComponent` (adding also adds a `RectTransform`). Without a Mask,
`GetShowMaskGraphic` returns `false` and the setter is a no-op.

| Function | Signature | Returns |
|---|---|---|
| `Mask.GetShowMaskGraphic` / `SetShowMaskGraphic` | `(id)` / `(id, bool)` | whether the mask's own graphic also draws (default `true`); off, it only shapes the clip |

A round minimap — a render texture shown through an ellipse:

```lua
local frame = UI.Create("Image", hud)            -- place and size its rect
Scene.RemoveComponent(frame, "Image")            -- an Image would win over the Shape
Scene.AddComponent(frame, "Shape")               -- an ellipse filling the rect
Shape.SetKind(frame, "Ellipse")
Scene.AddComponent(frame, "Mask")
Mask.SetShowMaskGraphic(frame, false)
local map = UI.Create("Image", frame)            -- what the minimap camera sees
Image.SetTexture(map, "rt:minimap")
```
