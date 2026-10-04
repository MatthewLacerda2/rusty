## `Graphics`

Live control over the engine's **existing** post-FX and quality knobs, so a script
can drive its own settings logic. The per-volume getters/setters target the first
**active** `VisualCorrectionComponent` (color/bloom/SSR/shadows/SSAO/custom effects and their params) and the first **active**
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
| `Graphics.GetFogMode` / `SetFogMode` | `()` / `(name)` | `"Off"` / `"Linear"` / `"Exponential"` / `"ExponentialSquared"` (**default `"Off"`**) |
| `Graphics.GetFogColor` / `SetFogColor` | `()` / `(r, g, b)` | `r, g, b` (linear, each clamped ≥ 0) |
| `Graphics.GetFogDensity` / `SetFogDensity` | `()` / `(value)` | `number` (per world unit, clamped ≥ 0, **default `0.02`**) |
| `Graphics.GetFogStart` / `SetFogStart` | `()` / `(units)` | `number` (clamped ≥ 0, **default `10`**) |
| `Graphics.GetFogEnd` / `SetFogEnd` | `()` / `(units)` | `number` (clamped ≥ 0, **default `100`**; linear mode only) |
| `Graphics.GetFogHeightFalloff` / `SetFogHeightFalloff` | `()` / `(value)` | `number` (clamped ≥ 0, **default `0`** = uniform) |
| `Graphics.GetFogBaseHeight` / `SetFogBaseHeight` | `()` / `(y)` | `number` (world height, **default `0`**) |
| `Graphics.GetSsaoActive` / `SetSsaoActive` | `()` / `(bool)` | `bool` (**default `true`** on a volume; `false` with no active volume) |
| `Graphics.GetSsaoRadius` / `SetSsaoRadius` | `()` / `(units)` | `number` (world units, clamped ≥ 0.01, **default `0.5`**) |
| `Graphics.GetSsaoIntensity` / `SetSsaoIntensity` | `()` / `(value)` | `number` (clamped 0–4, **default `1`**) |
| `Graphics.GetCustomEffects` / `SetCustomEffects` | `()` / `({names})` | array of baked postfx module names, in run order (**default empty**; empty with no active volume) |
| `Graphics.GetPostParam` / `SetPostParam` | `(name)` / `(name, value)` | a custom effect's runtime param: `number`, or an array for a vector param (the effect's baked default until set; the block's catalog default with no active volume, where a set raises an error) |

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

**Fog** (#437) is a **scene setting**, not a volume knob: it is saved with the
scene, edited in the Inspector's **Scene Settings → Fog** section (the same fields),
and the `Fog*` functions work whether or not the scene has a visual-correction
volume. One shared shader function applies it to **every world-space pass** —
opaque and transparent surfaces, unlit surfaces, alpha-blended and additive
particles, decals — so smoke and bullet holes fade exactly like the wall behind
them, and the sky blends to the fog colour at the horizon (fading out toward the
zenith) so fogged geometry never silhouettes against a clear sky. It runs in linear
HDR before post-FX; the in-game UI is never fogged.

- **Mode** sets how fog grows with distance past **start**: `Linear` ramps from
  none at `start` to full at `end`; `Exponential` is `1 − e^(−density·d)`;
  `ExponentialSquared` is `1 − e^(−(density·d)²)` (clear up close, then closing in
  fast). `Off` disables it everywhere.
- **Height**: below **base height** fog is at full thickness; above it, it thins
  by `e^(−falloff·h)`. A falloff of `0` is uniform fog; `0.2`–`1` pools it low, like
  smoke on a warehouse floor. The thickness is averaged along each view ray, so
  looking down into a fogged pit from above reads right.

The shader-authoring block `height_fog` (`Shader.md`) is a separate, per-material
*look*; every authored surface variant is fogged by the scene fog on top of it.
Fog is render-only, so it cannot affect the deterministic sim.

**SSAO** (screen-space ambient occlusion, #436) darkens the *ambient* light where
geometry crowds a point — the strip of floor at a crate's foot, the inside corner of
a room, the gap behind a door frame — which probes are too sparse to see. It touches
only the ambient term and the environment reflection, never the sun, a lamp or a
spotlight, so a lit wall stays lit; it is computed before the forward pass, not over
the finished image, so fog (applied after lighting) fogs the occluded colour rather
than AO darkening the fog. **Radius** is how far (world units) another surface can be and
still shade a point; **intensity** is the strength (`0` none, `1` natural, up to `4`
exaggerated). The knobs live on the active visual-correction volume (the Inspector's
**Ambient Occlusion** section edits the same fields) and are saved with the scene;
scenes saved before the knob existed load with it on. Without an active volume AO
does not run.

Its cost is gated by the quality preset: **off on Low**; on **Medium** 8 samples
per texel at half resolution; on **High** 16 at full resolution. Either way each
camera also draws its visible solids once more, depth only (a prepass), and runs a
25-tap blur at full resolution. The frame stats report it (`ssao_samples`, and the
prepass inside `draw_calls` / `triangles`; see `Debug`). `SetSsaoActive(false)` —
or intensity `0` — skips every one of those passes.

**Custom effects** (#397) are authored postfx modules — `Shader.Bake` with
`pass = "postfx"` (`Shader.md`) — run by the post chain, the way Unity's volume
stack runs custom post effects: damage vignettes, a low-health grayscale, CRT
scanlines on a menu camera.

```lua
Shader.Bake({ pass = "postfx", name = "hurt",
  blocks = { { id = "vignette", params = { strength = 0.8, radius = 0.6 } },
             { id = "tint", params = { color = {1.0, 0.6, 0.6} } } } })
Graphics.SetCustomEffects({ "hurt" })   -- next frame: vignetted, reddened
Graphics.SetCustomEffects({})           -- off again
```

- **Order and placement.** The list runs in order, **after tonemapping and before
  FXAA**: on the display-referred image in `[0, 1]` that grades like these are
  designed for, and before anti-aliasing so an effect's own hard edges (scanlines,
  posterize bands) are smoothed like the scene's. Each entry is one fullscreen pass;
  names may repeat.
- **Names** are what `Shader.Bake` was given (the file is `<name>.wgsl` in the
  default authored-shader dir, `project/assets/shaders`). `SetCustomEffects` raises
  an error — and changes nothing — for an empty name or one that is a path (`/`,
  `\`, a leading `.`). A name not baked yet is accepted.
- **Fail-safe.** A module that is missing, fails to compile, or doesn't fit the post
  pass is logged once and **skipped**; the rest of the list still runs and the frame
  never goes black. Re-baking any module makes the renderer reload the list next
  frame, so a fixed or edited effect is picked up live.
- **What a module sees.** Binding 1 is the colour so far (binding 2 its sampler);
  binding 3 is **the previous frame's** result of this chain, for feedback effects
  (black on the first frame and after a resize; after the list was empty for a
  while, the last frame it ran); bindings 0, 4 and 5 are the post
  params, scene depth and skybox, as for the built-in passes; group 1 is the
  module's runtime params (see *Post params* below).
- **Where it lives.** The list is saved with the volume, and the Inspector's
  **Custom Effects** section (add by name, reorder, remove) edits the same field. No
  active volume, no custom effects.
- **Cost.** One fullscreen pass per effect, plus one copy that keeps the history
  (and, with FXAA off, one more that lands the result on screen). An empty list
  costs nothing.

**Post params** (#671) drive a custom effect's **runtime** block params per frame —
the strengths a game fades with gameplay (`*` in `Shader.md`'s postfx list):

```lua
Shader.Bake({ pass = "postfx", name = "hurt",
  blocks = { { id = "damage_vignette", params = { color = {0.8, 0, 0} } } } })
Graphics.SetCustomEffects({ "hurt" })
function Update()
  Graphics.SetPostParam("damage_vignette.intensity", 1 - health / maxHealth)
end
```

- **Where they live.** On the active volume (`post_params`, saved with the scene
  like its other knobs), Unity's volume-profile overrides without the blending. A
  set is a uniform write the next frame packs — never a re-bake.
- **Names** are `"<block>.<param>"`, or `"<block>.<index>.<param>"` (the block's
  position in its recipe, required when it appears more than once) — the scheme
  `Material.SetShaderParam` uses. One name addresses the **whole list**: every
  listed effect whose recipe has that runtime param draws with the value.
- **Strict.** `SetPostParam` raises an error, and stores nothing, for a name no
  listed effect exposes at runtime (it names the culprit, says when the param is
  baked, and lists the runtime params there are), a wrong number of values (one
  number broadcasts to every lane), or no active volume. So list and bake the
  effect before setting its params. `GetPostParam` answers with the stored value,
  else the effect's baked default; with no active volume, the block's catalog
  default.

**FXAA** is the anti-aliasing pass at the very end of the chain, running on the
tonemapped image just before it reaches the screen. It is **on by default** — a
scene saved before the knob existed loads with it on, and so does a scene with no
camera component at all, which is why `GetFxaaActive` returns `true` rather than
`false` when there is no active camera. It is *not* gated by the quality preset:
it runs on every tier including Low, where a cheap fullscreen pass is exactly the
anti-aliasing an integrated GPU wants. Turning it off restores raw, stair-stepped
edges — useful for a pixel-art look, or for diffing screenshots.

The global **quality preset** gates the heavier passes (SSR is High-tier only;
motion blur and SSAO are off on Low, SSAO is half-resolution on Medium; FXAA runs
on all of them). `SetQuality` writes a shared
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
