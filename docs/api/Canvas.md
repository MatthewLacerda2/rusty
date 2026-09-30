## `Canvas`

Read and tune an entity's `CanvasComponent` — the root of an in-game UI (#417).
A screen canvas spans the whole screen; its descendants carrying a `RectTransform`
are laid out inside it. A `WorldSpace` canvas is a quad in the scene and a
`ScreenSpaceCamera` canvas a plane in front of the camera (#429). The scaler is Unity's *Scale With Screen Size*: layout runs in
**reference units** (default 1920×1080) and the scale factor maps them onto the
live screen. The model — coordinates, draw order, determinism — is in
[`docs/ui.md`](../ui.md). Getters return a neutral default (`0`, `"None"`, `nil`) when
the entity has no canvas.

| Function | Signature | Returns |
|---|---|---|
| `Canvas.GetRenderMode` / `SetRenderMode` | `(id)` / `(id, name)` | `"ScreenSpaceOverlay"`, `"ScreenSpaceCamera"` or `"WorldSpace"` |
| `Canvas.GetSortOrder` / `SetSortOrder` | `(id)` / `(id, order)` | integer; higher draws on top and is hit first |
| `Canvas.GetReferenceResolution` / `SetReferenceResolution` | `(id)` / `(id, w, h)` | `w, h` (each clamped ≥ 1) |
| `Canvas.GetMatchWidthOrHeight` / `SetMatchWidthOrHeight` | `(id)` / `(id, value)` | `0` = match width … `1` = match height (clamped) |
| `Canvas.GetScaleFactor` | `(id)` | screen pixels per reference unit on the current screen (`1` for `WorldSpace`), or `nil` |
| `Canvas.GetPixelsPerUnit` / `SetPixelsPerUnit` | `(id)` / `(id, value)` | `WorldSpace`: reference units per metre (default 100; kept > 0) |
| `Canvas.GetPlaneDistance` / `SetPlaneDistance` | `(id)` / `(id, metres)` | `ScreenSpaceCamera`: the plane's distance in front of the camera (default 1; ≥ 0.01) |
| `Canvas.GetTilt` / `SetTilt` | `(id)` / `(id, x, y)` | `ScreenSpaceCamera`: degrees — `x` leans the top away, `y` the right edge (each ±89) |
| `Canvas.GetSway` / `SetSway` | `(id)` / `(id, amount)` | `ScreenSpaceCamera`: how much the canvas lags a camera turn, `0` … `1` |

`SetRenderMode` is case-insensitive (`"Overlay"`, `"Camera"` and `"World"` also
work); an unrecognized name is ignored.

**Render modes** (details in [`docs/ui.md`](../ui.md#world-space-ui)):

- `ScreenSpaceOverlay` — over the finished frame; the HUD and menus.
- `ScreenSpaceCamera` — laid out exactly like the overlay, but drawn on a plane
  `PlaneDistance` metres in front of the active camera that fills the view there:
  in the scene (depth-tested and tonemapped like the world), tiltable (`SetTilt`) and swaying
  behind camera turns (`SetSway`) — a visor HUD. Without a camera it is the overlay.
- `WorldSpace` — a quad in the scene: its rect is `ReferenceResolution` reference
  units, `PixelsPerUnit` to the metre, centred on the canvas's Transform and facing
  its +Z (readable from the +Z side). Walls occlude it. Terminals, signs, a gun's
  ammo counter (parent the canvas to the gun).

A world or camera canvas's graphics are hit by the camera ray through the pointer —
the screen centre while the cursor is locked — so `OnPointerClick` works on a
terminal you look at and click. A world-space health bar should turn its images'
`raycast_target` off, or it catches the pointer (and `UI.IsPointerConsumed`) when
the crosshair crosses it.
