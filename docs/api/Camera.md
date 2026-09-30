## `Camera`

| Function | Signature | Returns |
|---|---|---|
| `Camera.GetPosition` | `()` | `x, y, z` |
| `Camera.SetPosition` | `(x, y, z)` | — |
| `Camera.GetForward` | `()` | `x, y, z` (unit look direction) |
| `Camera.GetRight` | `()` | `x, y, z` (unit right vector) |
| `Camera.GetYaw` / `SetYaw` | `()` / `(yaw)` | degrees |
| `Camera.GetPitch` / `SetPitch` | `()` / `(pitch)` | degrees (clamped ±89) |
| `Camera.GetFov` / `SetFov` | `()` / `(fov)` | degrees (clamped 1–179) |
| `Camera.WorldToScreen` | `(x, y, z [, canvas])` | `sx, sy, depth, onScreen` — see below |
| `Camera.ScreenToWorldRay` | `(x, y)` | `ox, oy, oz, dx, dy, dz` — the ray's origin (on the near plane) and unit direction |

**World ↔ screen (#429).** Screen points are **UI screen pixels** — bottom-left
origin, y-up, the frame of `UI.GetRect(id).screen` and the UI pointer (Unity's
screen space; `Input.GetMousePosition` is top-left, so flip `y`). The screen is the
sim's (`UI.GetScreenSize`), so the answer is the same headless as in the window.

- `WorldToScreen` projects a world point. Pass a canvas id to get that canvas's
  **reference units** instead of pixels (pixels ÷ its scale factor). `depth` is the
  distance in front of the camera along its view — `≤ 0` means **behind** it, where
  the point projects mirrored through the centre; `onScreen` is `true` only in front
  and inside the screen.
- `ScreenToWorldRay` is the ray through a screen pixel — feed it to
  `Physics.Raycast` to pick what is under the mouse, or use `UI.GetScreenSize()`'s
  centre for a crosshair.

```lua
local x, y, depth, onScreen = Camera.WorldToScreen(ex, ey + 2, ez, hud)
if onScreen then RectTransform.SetAnchoredPosition(label, x, y) end
```

For markers that follow an entity every tick, prefer a world anchor
(`RectTransform.SetWorldAnchor`) — the layout does this for you, with edge clamping.
