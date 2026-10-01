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

### Camera entities: projection and render textures (#430)

The functions above drive the **view** camera. These take an entity id and tune that
entity's `CameraComponent` — the same fields as the inspector's Camera card. Getters
return `nil` without a camera; setters are then no-ops.

| Function | Signature | Returns |
|---|---|---|
| `Camera.GetProjection` / `SetProjection` | `(id)` / `(id, name [, size])` | `name, size` — `"Perspective"` (size `nil`) or `"Orthographic"`; `size` is half the view's height in world units (Unity's `orthographicSize`, > 0; omitted keeps the current one, else 5) |
| `Camera.GetTargetTexture` / `SetTargetTexture` | `(id)` / `(id, name [, width, height])` | `name, width, height`, or `nil` for a screen camera; `nil` / `""` clears the target. Sides clamp to 1..4096; omitted ones keep the current size, else 256 |
| `Camera.GetTargetPostFx` / `SetTargetPostFx` | `(id)` / `(id, bool)` | full post-FX chain (default) or tonemap only; `nil` / no-op without a target |
| `Camera.GetTargetUpdateEvery` / `SetTargetUpdateEvery` | `(id)` / `(id, n)` | redraw every `n`th frame (≥ 1, default 1); `nil` / no-op without a target |

**Render textures** (Unity's `Camera.targetTexture` + `RawImage`). A camera with a
target leaves the screen stack and draws into a texture named `name`, from **its own
entity's transform**; anything that takes a texture path shows it as `"rt:<name>"` —
a UI `Image` (`Image.SetTexture(id, "rt:minimap")`) or a material map
(`Material.SetEmissiveMap(screen, "rt:securityCam")`) for an in-world monitor.

- **Frame order.** Texture cameras draw before the screen, in `render_order`, so the
  screen shows this frame's picture. A camera that sees a monitor showing its own
  texture sees *last* frame's picture there (no feedback hazard).
- **Shadows** come from the screen camera's cascades: near the player — where a
  minimap, scope or preview looks — they match the screen; beyond them the texture
  is unshadowed. Screen-space UI is not drawn into a texture.
- **Cost.** A texture camera draws only when an active Image or material references
  its texture, and only every `update_every` frames (the texture holds its picture
  in between). Its draws count in `Debug.Stats()` like any camera's, plus
  `render_texture_draws`.
- **Edit mode** (the Scene view) draws no texture cameras; play mode, the Game view
  and headless screenshots do.
- The texture is display-referred (8-bit sRGB, like a screen): post-FX is where HDR
  is resolved, so `SetTargetPostFx(id, false)` still tonemaps.
- A camera rendering from its own transform applies to **stacked screen cameras**
  too: every camera after the base one (the lowest `render_order`, whose pose the
  view camera supplies) renders from its own entity — parent a viewmodel camera to
  the player's head and it follows.

```lua
-- A top-down minimap in the HUD's corner.
Camera.SetProjection(minimapCam, "Orthographic", 40)
Camera.SetTargetTexture(minimapCam, "minimap", 256, 256)
Camera.SetTargetPostFx(minimapCam, false)
Image.SetTexture(minimapImage, "rt:minimap")
```
