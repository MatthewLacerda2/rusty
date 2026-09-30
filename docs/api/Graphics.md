## `Graphics`

Live control over the engine's **existing** post-FX and quality knobs, so a script
can drive its own settings logic. The per-volume getters/setters target the first
**active** `VisualCorrectionComponent` (color/bloom/SSR) and the first **active**
`CameraComponent` (motion blur) in the scene — the same volume `build_post_params`
packs into the GPU uniform each frame, so a write here takes effect next frame.
Getters return a neutral default when no active volume/camera exists.

| Function | Signature | Returns |
|---|---|---|
| `Graphics.GetBloomActive` / `SetBloomActive` | `()` / `(bool)` | `bool` |
| `Graphics.GetBloomIntensity` / `SetBloomIntensity` | `()` / `(value)` | `number` (≥ 0) |
| `Graphics.GetBloomThreshold` / `SetBloomThreshold` | `()` / `(value)` | `number` (≥ 0) |
| `Graphics.GetExposure` / `SetExposure` | `()` / `(ev)` | `number` (EV) |
| `Graphics.GetContrast` / `SetContrast` | `()` / `(value)` | `number` |
| `Graphics.GetSaturation` / `SetSaturation` | `()` / `(value)` | `number` |
| `Graphics.GetGamma` / `SetGamma` | `()` / `(value)` | `number` (display-gamma tweak, **neutral `1.0`**, clamped ≥ 0.01) |
| `Graphics.GetTonemap` / `SetTonemap` | `()` / `(name)` | `"None"` / `"Reinhard"` / `"Aces"` |
| `Graphics.GetSsrActive` / `SetSsrActive` | `()` / `(bool)` | `bool` |
| `Graphics.GetSsrQuality` / `SetSsrQuality` | `()` / `(name)` | `string` |
| `Graphics.GetMotionBlurActive` / `SetMotionBlurActive` | `()` / `(bool)` | `bool` |
| `Graphics.GetMotionBlurSamples` / `SetMotionBlurSamples` | `()` / `(n)` | `number` (clamped 2–32) |
| `Graphics.GetFxaaActive` / `SetFxaaActive` | `()` / `(bool)` | `bool` (**default `true`**) |
| `Graphics.GetQuality` / `SetQuality` | `()` / `(name)` | `"Low"` / `"Medium"` / `"High"` |
| `Graphics.GetShadowCascades` / `SetShadowCascades` | `()` / `(n)` | `number` (clamped 1–4, **default `4`**) |
| `Graphics.GetShadowDistance` / `SetShadowDistance` | `()` / `(units)` | `number` (world units, clamped ≥ 1, **default `100`**) |

**Gamma** is a display-gamma *tweak*, not the output encode: the render target
(window, Game view, and headless screenshot alike) is sRGB and encodes linear →
display in hardware, exactly once (#415). `1.0` leaves the image untouched; `> 1`
lifts the midtones, `< 1` darkens them. Scenes saved before #415 stored the
encode itself (`gamma: 2.2` meant "normal"); they load as `gamma / 2.2`, so an old
2.2 becomes 1.0 and looks the same.

**Sun shadows** are cascaded (#435): the camera's view, out to the **shadow
distance**, is split into `1`–`4` slices, each with its own shadow map fitted to it
and following the camera, so the first few metres get the sharpest shadow and a
caster anywhere in a level still casts. Past the distance shadows fade out over its
last tenth. Both knobs live on the active visual-correction volume (the Inspector's
**Shadows** section edits the same fields) and are saved with the scene, so an
indoor level can spend its cascades on 40 m and an outdoor one on 150 m; with no
active volume the defaults above apply. Each cascade costs one more depth pass for
the moving casters (static casters are re-drawn only when a cascade moves, #355).

**FXAA** is the anti-aliasing pass at the very end of the chain, running on the
tonemapped image just before it reaches the screen. It is **on by default** — a
scene saved before the knob existed loads with it on, and so does a scene with no
camera component at all, which is why `GetFxaaActive` returns `true` rather than
`false` when there is no active camera. It is *not* gated by the quality preset:
it runs on every tier including Low, where a cheap fullscreen pass is exactly the
anti-aliasing an integrated GPU wants. Turning it off restores raw, stair-stepped
edges — useful for a pixel-art look, or for diffing screenshots.

The global **quality preset** gates the heavier passes (SSR is High-tier only;
motion blur is off on Low; FXAA runs on all of them). `SetQuality` writes a shared
resource cell that the platform layer reads each frame and hands to the renderer,
which reallocates its bloom buffers when the tier actually changes. Unrecognized
tonemap/quality names are ignored (the current value is kept).

**Determinism.** Every `Graphics` write is **one-way** into render-only state: the
post-FX volume and the preset cell are read by the render layer, never by
`FixedUpdate`. Toggling these knobs therefore cannot change how the deterministic
sim evolves, and a headless replay stays bit-for-bit stable regardless of them.

The quality preset and the `Video` settings below are **persisted** through
`Storage` (the `graphics.quality` key and the `video` namespace): the windowed app
reads them at startup and writes them back at the Stop / quit boundary, so the app
relaunches at the last-chosen tier and video settings.
