## `BackdropFilter`

Read and tune an entity's `BackdropFilterComponent` (#426) — frosted glass, CSS's
`backdrop-filter`. The game frame behind the entity's graphic is blurred,
desaturated, brightened and tinted, and shown through the graphic's shape (its
`Image`'s texture alpha, or the rect without one), under the graphic itself. The
graphic's colour alpha does not hide the backdrop, so an `Image` with alpha `0` is
pure glass and a dark, half-transparent one darkens it further. The backdrop is the
finished 3D frame (graded, tonemapped, bloomed), not UI drawn before it.
**Screen-space (`ScreenSpaceOverlay`) canvases only**: on a world or camera canvas
the component is ignored and the graphic draws as if it had none. Add or remove
one with `Scene.AddComponent(id, "BackdropFilter")` / `RemoveComponent` (adding
also adds a `RectTransform`). Getters return the defaults without one; setters
are then no-ops.

| Function | Signature | Returns |
|---|---|---|
| `BackdropFilter.GetBlurRadius` / `SetBlurRadius` | `(id)` / `(id, units)` | blur radius in reference units, `>= 0` (default `16`; `0`: unblurred) |
| `BackdropFilter.GetTint` / `SetTint` | `(id)` / `(id, r, g, b, a)` | colour mixed over the backdrop by its alpha (default transparent) |
| `BackdropFilter.GetSaturation` / `SetSaturation` | `(id)` / `(id, s)` | `1` unchanged, `0` greyscale, `>= 0` |
| `BackdropFilter.GetBrightness` / `SetBrightness` | `(id)` / `(id, b)` | multiplier, `< 1` darkens, `>= 0` |

A pause screen on a blurred, darkened game:

```lua
local shade = UI.Create("Image", pause_canvas)   -- stretch it over the canvas
Image.SetColor(shade, 0, 0, 0, 0.25)
Scene.AddComponent(shade, "BackdropFilter")
BackdropFilter.SetBlurRadius(shade, 24)
BackdropFilter.SetSaturation(shade, 0.6)
```
