## `Video`

Runtime **video** settings for the windowed app — framebuffer resolution, vsync, and
fullscreen. Kept separate from `Graphics` (which drives per-volume post-FX + the
quality tier) because these are window/surface state, not scene-graph state — the
same split Unity draws between `Screen` and `QualitySettings`.

| Function | Signature | Returns |
|---|---|---|
| `Video.GetDisplayScale` | `()` | `number`: the display's backing scale, physical pixels per point (`2` on a Retina display; `1` headless) |
| `Video.GetResolution` / `SetResolution` | `()` / `(width, height)` | `width, height` (each clamped ≥ 1) |
| `Video.GetVsync` / `SetVsync` | `()` / `(bool)` | `bool` |
| `Video.GetFullscreen` / `SetFullscreen` | `()` / `(bool)` | `bool` |

A `Video.*` setter writes a shared settings cell that the platform layer reads each
frame and applies: resolution and vsync reconfigure the wgpu surface, fullscreen
toggles the winit window (borderless). Vsync off requests the surface's `Immediate`
present mode; if the backend doesn't support it, vsync stays on and `GetVsync`
reports the true state.

**`SetResolution` sizes the window or the framebuffer, never the scene alone.**
Windowed, it asks the OS to resize the window (in the editor, the editor's window),
and the surface keeps the display's backing scale: on a 2× Retina display it still
has two pixels per point, and the scene draws at every one of them. Fullscreen, the
window cannot resize, so it reconfigures the surface to `w × h`: the frame does
render fewer pixels, but the UI with it, so the HUD softens as much as the scene. To
render the 3D scene at fewer pixels and keep the HUD native, set
[`Graphics.SetRenderScale`](Graphics.md) (Unity's *Render Scale*): `0.5` on a 2×
surface renders the scene at the 1× pixel count, and the UI stays sharp; a project
default of `"match_display"` (`Graphics.SetDefaultRenderScale`) does it on every
HiDPI display.

**In the editor, the Game view's size is the editor's, not `SetResolution`'s.**
`SetResolution` there resizes the whole editor window, and the Game view stays the
panel it sits in. To play at the size the game ships at, give the Game view a size of
its own: its dropdown on the Game tab's strip, or
[`Editor.SetGameViewSize`](Editor.md#the-game-views-size) (Unity's Game view
resolution menu). The scene, the post chain, TAA's output, the UI and the mouse
position then all use that size, drawn letterboxed in the panel. A fixed size is a
pixel count, so `GetDisplayScale` reads `1` there (a `"match_display"` render scale
renders it natively); with its "1x on HiDPI" choice the Game view is sized by the
panel's points, and `GetDisplayScale` reads `1` too.

**`GetDisplayScale`** (#1715; Unity reads the same through `Screen.dpi`) tells a
Retina 2940×1388 from a 1× monitor of the same pixels. The window reports it each
frame, so moving the window to another monitor changes it the next frame. Like
`UI.GetScreenSize` it is a sim input: headless it is `1`, and a demo records it, so
a replay reads the recording's scale, not the monitor it plays on.

Settings persist through the `video` `Storage` namespace
(loaded at startup, written back when the app quits; Stop does not store them), so
the app relaunches as last set. In
the headless harness there is no window/surface to resize, and its `Storage` opens no
file, so a run starts at the default 1280x720 whatever the machine last saved.

**Determinism.** The screen size is a **sim input**: `UI.GetScreenSize`, canvas
layout and `Camera.WorldToScreen` / `ScreenToWorldRay` read it inside `FixedUpdate`.
With a window it is the Game view's size, which the platform writes each frame;
headless there is none, and the size is the resolution `SetResolution` last set
(`play --size` gives the run a Game view instead). Neither ever comes from the
machine's saved settings headless, and a demo records both: the Game view's size at
each change, and the save data with its `video` namespace, so a replay lays its UI out
on the recording's screen, not the one it plays on. Vsync and fullscreen are one-way
into the platform and never read by the sim.
