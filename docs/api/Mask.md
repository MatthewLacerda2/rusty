## `Mask`

Read and tune an entity's `MaskComponent` (#428) — Unity's `Mask`. Every graphic
*below* the entity is clipped to the entity's own graphic: its `Image`'s texture
alpha times its colour alpha, so a circle sprite gives a round minimap and a
soft-edged sprite a feathered edge (without an `Image` the clip is the rect).
Nested masks multiply, to any depth, and a `RectMask` above still applies. The
pointer cannot hit a child outside the mask's rect. Works on screen and world
canvases alike. Add or remove one with `Scene.AddComponent(id, "Mask")` /
`RemoveComponent` (adding also adds a `RectTransform`). Without a Mask,
`GetShowMaskGraphic` returns `false` and the setter is a no-op.

| Function | Signature | Returns |
|---|---|---|
| `Mask.GetShowMaskGraphic` / `SetShowMaskGraphic` | `(id)` / `(id, bool)` | whether the mask's own graphic also draws (default `true`); off, it only shapes the clip |

A round minimap — a render texture shown through a circle:

```lua
local frame = UI.Create("Image", hud)            -- the circle sprite is the mask
Image.SetTexture(frame, "assets/ui/circle.png")
Scene.AddComponent(frame, "Mask")
Mask.SetShowMaskGraphic(frame, false)
local map = UI.Create("Image", frame)            -- what the minimap camera sees
Image.SetTexture(map, "rt:minimap")
```
