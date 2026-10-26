## `Graphics`

Live control over the engine's **existing** post-FX and quality knobs, so a script
can drive its own settings logic. The per-volume getters/setters target the **main
camera**, Unity's `Camera.main`: the base of the screen stack, the lowest
`render_order` active camera that renders to the screen (a render-texture camera never
is). Its `PostProcessingComponent` (color/bloom/SSR/shadows/SSAO/custom effects and
their params) and its `CameraComponent` (motion blur, FXAA) are what the window shows,
so a write takes effect there next frame. With no screen camera, or its volume missing
or inactive, setters are no-ops and getters report what a new volume starts with (an
effect's on/off switch reads `false`: nothing runs).
The editor's Camera card edits the same four motion-blur fields (a "Motion Blur"
checkbox and "Samples" `2..=32`, "Intensity" `0..=1` and "Clamp" `0..=0.2` sliders)
through the same ops. A new camera starts with motion blur on at 32 samples, intensity
`0.5` and clamp `0.05`. With no screen camera nothing blurs, and the four motion-blur
getters say so: `false`, `0`, `0` and `0`.

**Post-FX is per view** (#1434), Unity URP's camera-stack rule: each view runs it once,
with the settings of **its base camera**, that camera entity's own `CameraComponent`
and `PostProcessingComponent`. The window and the Game view use the main camera;
overlay cameras stacked on it add no post-FX of their own. A render-texture camera
uses its own. The Scene view's free camera has no entity, so it previews the main
camera. A camera with no active volume runs neutral (no grade, bloom or custom
effects; FXAA still follows its `fxaa_active`), so a scene whose only volume sits on
a render-texture camera shows a neutral window and Scene view. `Graphics.*` writes
reach the window only; another camera's settings (a security monitor's grade) are
set on that camera entity, in the inspector or the scene file.

<!-- sorted: one row per function pair, by name; the sections below say what each does -->
| Function | Signature | Returns |
|---|---|---|
| `Graphics.GetAdaptationSpeed` / `SetAdaptationSpeed` | `()` / `(up, down)` | `up, down` in EV per second (each clamped 0–100; **default `1, 3`**) |
| `Graphics.GetBloomActive` / `SetBloomActive` | `()` / `(bool)` | `bool` |
| `Graphics.GetBloomIntensity` / `SetBloomIntensity` | `()` / `(value)` | `number` (≥ 0) |
| `Graphics.GetBloomThreshold` / `SetBloomThreshold` | `()` / `(value)` | `number` (≥ 0) |
| `Graphics.GetChromaticAberrationActive` / `SetChromaticAberrationActive` | `()` / `(bool)` | `bool` (**default `false`**) |
| `Graphics.GetChromaticAberrationIntensity` / `SetChromaticAberrationIntensity` | `()` / `(value)` | `number` (clamped 0–1, **default `0.3`**) |
| `Graphics.GetColorLookup` / `SetColorLookup` | `()` / `(path)` | `string` or `nil`: the user LUT file (a `.cube`, or a Unity strip `.png`; `nil` or `""` clears it; see *Color Lookup*) |
| `Graphics.GetColorLookupContribution` / `SetColorLookupContribution` | `()` / `(value)` | `number` (how much of the LUT applies, clamped 0–1, **default `1`**) |
| `Graphics.GetContrast` / `SetContrast` | `()` / `(value)` | `number` |
| `Graphics.GetCustomEffects` / `SetCustomEffects` | `()` / `({names})` | array of baked postfx module names, in run order (**default empty**; empty with no active volume) |
| `Graphics.GetDefaultOption` / `SetDefaultOption` | `(name)` / `(name, bool)` | `bool`: the project's default for an option |
| `Graphics.GetDepthOfFieldAperture` / `SetDepthOfFieldAperture` | `()` / `(f_stop)` | `number`: Bokeh's f-stop, lower blurs more (clamped 1–32, **default `5.6`**) |
| `Graphics.GetDepthOfFieldFocalLength` / `SetDepthOfFieldFocalLength` | `()` / `(mm)` | `number`: Bokeh's focal length, longer blurs more (clamped 1–300, **default `50`**) |
| `Graphics.GetDepthOfFieldFocusDistance` / `SetDepthOfFieldFocusDistance` | `()` / `(metres)` | `number`: Bokeh's distance in focus, along the view axis (clamped to `0.1`–`10000`; `math.huge` is focus at infinity; **default `10`**) |
| `Graphics.GetDepthOfFieldGaussianEnd` / `SetDepthOfFieldGaussianEnd` | `()` / `(metres)` | `number`: where it is full (clamped ≥ 0; an end nearer than the start is drawn at the start, an instant edge; **default `30`**) |
| `Graphics.GetDepthOfFieldGaussianMaxRadius` / `SetDepthOfFieldGaussianMaxRadius` | `()` / `(value)` | `number`: the full blur's width (clamped 0.5–1.5, **default `1`**) |
| `Graphics.GetDepthOfFieldGaussianStart` / `SetDepthOfFieldGaussianStart` | `()` / `(metres)` | `number`: where the Gaussian blur starts (clamped ≥ 0, independent of the end, as in Unity; **default `10`**) |
| `Graphics.GetDepthOfFieldMode` / `SetDepthOfFieldMode` | `()` / `(name)` | `"Off"` / `"Gaussian"` / `"Bokeh"` (**default `"Off"`**; names are case-insensitive, an unknown one raises an error listing them; see *Depth of field*) |
| `Graphics.GetExposure` / `SetExposure` | `()` / `(ev)` | `number` (EV): the configured exposure, in both modes (the adapted EV in `Automatic` is the dev-only `Debug.GetAdaptedExposure`; see *Automatic exposure*) |
| `Graphics.GetExposureCompensation` / `SetExposureCompensation` | `()` / `(ev)` | `number` (clamped −10–10, **default `0`**) |
| `Graphics.GetExposureLimits` / `SetExposureLimits` | `()` / `(min_ev, max_ev)` | `min_ev, max_ev` (each clamped −10–10, `max_ev` ≥ `min_ev`; **default `-6, 6`**) |
| `Graphics.GetExposureMode` / `SetExposureMode` | `()` / `(name)` | `"Fixed"` / `"Automatic"` (**default `"Fixed"`**; names are case-insensitive) |
| `Graphics.GetFilmGrainActive` / `SetFilmGrainActive` | `()` / `(bool)` | `bool` (**default `false`**) |
| `Graphics.GetFilmGrainIntensity` / `SetFilmGrainIntensity` | `()` / `(value)` | `number` (clamped 0–1, **default `0.3`**) |
| `Graphics.GetFilmGrainResponse` / `SetFilmGrainResponse` | `()` / `(value)` | `number` (how much the grain fades in bright areas, clamped 0–1, **default `0.8`**) |
| `Graphics.GetFogBaseHeight` / `SetFogBaseHeight` | `()` / `(y)` | `number` (world height, **default `0`**) |
| `Graphics.GetFogColor` / `SetFogColor` | `()` / `(r, g, b)` | `r, g, b` (linear, each clamped ≥ 0) |
| `Graphics.GetFogDensity` / `SetFogDensity` | `()` / `(value)` | `number` (per world unit, clamped ≥ 0, **default `0.02`**) |
| `Graphics.GetFogEnd` / `SetFogEnd` | `()` / `(units)` | `number` (clamped ≥ 0, **default `100`**; linear mode only) |
| `Graphics.GetFogHeightFalloff` / `SetFogHeightFalloff` | `()` / `(value)` | `number` (clamped ≥ 0, **default `0`** = uniform) |
| `Graphics.GetFogMode` / `SetFogMode` | `()` / `(name)` | `"Off"` / `"Linear"` / `"Exponential"` / `"ExponentialSquared"` (**default `"Off"`**; names are case-insensitive) |
| `Graphics.GetFogStart` / `SetFogStart` | `()` / `(units)` | `number` (clamped ≥ 0, **default `10`**) |
| `Graphics.GetFxaaActive` / `SetFxaaActive` | `()` / `(bool)` | `bool` (**default `true`**) |
| `Graphics.GetGainWheel` / `SetGainWheel` | `()` / `(r, g, b, offset)` | `r, g, b, offset`: the Gain wheel, as the Lift wheel |
| `Graphics.GetGamma` / `SetGamma` | `()` / `(value)` | `number` (display-gamma tweak, **neutral `1.0`**, clamped ≥ 0.01) |
| `Graphics.GetGammaWheel` / `SetGammaWheel` | `()` / `(r, g, b, offset)` | `r, g, b, offset`: the Gamma wheel, as the Lift wheel. Not `GetGamma`, the display-gamma tweak |
| `Graphics.GetHistogramPercent` / `SetHistogramPercent` | `()` / `(low, high)` | `low, high` (clamped 0–100, `high` ≥ `low`; **default `10, 90`**) |
| `Graphics.GetHueShift` / `SetHueShift` | `()` / `(degrees)` | `number` (clamped −180–180, **neutral `0`**) |
| `Graphics.GetLensFlareActive` / `SetLensFlareActive` | `()` / `(bool)` | `bool` (**default `false`**; runs only while bloom does; see *Lens flare*) |
| `Graphics.GetLensFlareChromaticAberration` / `SetLensFlareChromaticAberration` | `()` / `(value)` | `number` (how far the flare's red and blue split, clamped 0–1, **default `0.5`**) |
| `Graphics.GetLensFlareGhostCount` / `SetLensFlareGhostCount` | `()` / `(n)` | `number` (ghosts per bright pixel, clamped 0–8, **default `4`**) |
| `Graphics.GetLensFlareGhostIntensity` / `SetLensFlareGhostIntensity` | `()` / `(value)` | `number` (clamped ≥ 0, **default `1`**) |
| `Graphics.GetLensFlareHaloIntensity` / `SetLensFlareHaloIntensity` | `()` / `(value)` | `number` (clamped ≥ 0, **default `0.5`**) |
| `Graphics.GetLensFlareIntensity` / `SetLensFlareIntensity` | `()` / `(value)` | `number` (the whole flare's strength, clamped ≥ 0, **default `1`**) |
| `Graphics.GetLensFlareStreakIntensity` / `SetLensFlareStreakIntensity` | `()` / `(value)` | `number` (horizontal streaks, clamped ≥ 0, **default `0`** = none) |
| `Graphics.GetLensFlareTint` / `SetLensFlareTint` | `()` / `(r, g, b)` | `r, g, b` (linear, each clamped 0–1, **default white**) |
| `Graphics.GetLiftWheel` / `SetLiftWheel` | `()` / `(r, g, b, offset)` | `r, g, b, offset`: the Lift wheel (colour each clamped 0–1, offset −1–1, **neutral `1, 1, 1, 0`**) |
| `Graphics.GetMotionBlurActive` / `SetMotionBlurActive` | `()` / `(bool)` | `bool` |
| `Graphics.GetMotionBlurIntensity` / `SetMotionBlurIntensity` | `()` / `(value)` | `number`: Unity's *Intensity*, the shutter fraction (clamped 0–1, **default `0.5`**, a 180° shutter) |
| `Graphics.GetMotionBlurMaxLength` / `SetMotionBlurMaxLength` | `()` / `(value)` | `number`: Unity's *Clamp*, the longest streak as a fraction of the view's height (clamped 0–0.2, **default `0.05`**) |
| `Graphics.GetMotionBlurSamples` / `SetMotionBlurSamples` | `()` / `(n)` | `number` (clamped 2–32; the clamp is in the shared authoring op) |
| `Graphics.GetOption` / `SetOption` | `(name)` / `(name, bool)` | `bool`: the player's graphics option (see *Player options*; **default `true`**) |
| `Graphics.GetOptionNames` | `()` | array of every option name, alphabetical (a menu's order) |
| `Graphics.GetPostParam` / `SetPostParam` | `(name)` / `(name, value)` | a custom effect's runtime param: `number`, or an array for a vector param (the effect's baked default until set; the block's catalog default with no active volume, where a set raises an error) |
| `Graphics.GetQuality` / `SetQuality` | `()` / `(name)` | `"Low"` / `"Medium"` / `"High"` |
| `Graphics.GetSaturation` / `SetSaturation` | `()` / `(value)` | `number` |
| `Graphics.GetShadowCascades` / `SetShadowCascades` | `()` / `(n)` | `number` (clamped 1–4, **default `4`**) |
| `Graphics.GetShadowDistance` / `SetShadowDistance` | `()` / `(units)` | `number` (world units, clamped ≥ 1, **default `100`**) |
| `Graphics.GetSsaoActive` / `SetSsaoActive` | `()` / `(bool)` | `bool` (**default `true`** on a volume; `false` with no active volume) |
| `Graphics.GetSsaoIntensity` / `SetSsaoIntensity` | `()` / `(value)` | `number` (clamped 0–4, **default `1`**) |
| `Graphics.GetSsaoRadius` / `SetSsaoRadius` | `()` / `(units)` | `number` (world units, clamped ≥ 0.01, **default `0.5`**) |
| `Graphics.GetSsrActive` / `SetSsrActive` | `()` / `(bool)` | `bool` |
| `Graphics.GetSsrQuality` / `SetSsrQuality` | `()` / `(name)` | `string` |
| `Graphics.GetTonemap` / `SetTonemap` | `()` / `(name)` | `"None"` / `"Reinhard"` / `"ACES"` (Unity's name; names are case-insensitive) |
| `Graphics.GetVignetteActive` / `SetVignetteActive` | `()` / `(bool)` | `bool` (**default `false`**; see *Lens effects*) |
| `Graphics.GetVignetteCenter` / `SetVignetteCenter` | `()` / `(x, y)` | `x, y` (screen uv, `0, 0` top-left, each clamped 0–1, **default `0.5, 0.5`**) |
| `Graphics.GetVignetteColor` / `SetVignetteColor` | `()` / `(r, g, b)` | `r, g, b` (linear, each clamped 0–1, **default black**) |
| `Graphics.GetVignetteIntensity` / `SetVignetteIntensity` | `()` / `(value)` | `number` (clamped 0–1, **default `0.3`**) |
| `Graphics.GetVignetteSmoothness` / `SetVignetteSmoothness` | `()` / `(value)` | `number` (falloff softness, clamped 0.01–1, **default `0.2`**) |
| `Graphics.GetWhiteBalance` / `SetWhiteBalance` | `()` / `(temperature, tint)` | `temperature, tint` (each clamped −100–100, **neutral `0, 0`**; see *Colour grading*) |
| `Graphics.ResetOptions` | `()` | nothing; every option back to the project's default |

### Post effects

Each post effect is one module in every layer of the engine (#1467): its settings on
the volume (motion blur and FXAA on the camera, as Unity keeps them), its ops, its
`Graphics.*` functions, its Inspector section, its render functions or pass, and its
section below. They run in this order: depth of field, motion blur, the exposure
meter, bloom's bright pass and the lens flare over the HDR scene; then the composite, which runs
chromatic aberration, screen-space reflections, bloom's add and the exposure on the
HDR colour, then the grading LUT (white balance, Lift / Gamma / Gain, the color
adjustments, the tonemapper, the display gamma and the Color Lookup, baked into one
table), then the vignette and film grain on the display colour; then the custom
effects and FXAA. An effect that is off (or at a zero strength) costs nothing and
leaves the image byte-identical.

**Tweening.** Every numeric knob of an effect tweens by its path inside the effect's
settings, `Tween.To(volume, "Graphics.<effect>.<knob>", to, seconds)` on the volume
entity `volume`, through the same op as its `Graphics.*` function and the Inspector's
slider, so it clamps the same way (see [`Tween.md`](Tween.md)). Each section lists its
paths; a switch, a mode or a name does not tween.

<!-- effect: depth_of_field -->
#### Depth of field

**Depth of field** (#1445) is Unity URP's override, in its two modes. **Gaussian**
blurs only the background: nothing nearer than the **start**, ramping to the full
**max radius** (`1` is 12 px at 1080p) at the **end**; cheap, for menus and
cutscenes. **Bokeh** is a lens: a thin-lens circle of confusion from the **focus
distance**, the **aperture** (f-stop) and the **focal length** on a 35 mm sensor, so
both what lies nearer and what lies beyond the focus blur, with round bokeh, up to a
radius of 1/40 of the view's height. Near blur spreads over what is in focus behind
it; far blur never bleeds over a sharp foreground. Aiming down sights is a tween of
the focus: `Tween.To(camera, "Graphics.depth_of_field.focus_distance", 1.5, 0.2)`
(every numeric knob tweens as `"Graphics.depth_of_field.<knob>"`, the knob's field
name; see [`Tween.md`](Tween.md)). It runs first in the chain, on the HDR scene,
before motion blur, bloom and the grade (Unity's order). Each pixel's blur comes from
its depth along the view axis of **the camera that drew it** (so the focus is a plane,
as in Unity: a wall at the focus distance is sharp to its corners), and a viewmodel on
a `DepthOnly` overlay camera (a gun 0.6 m from its camera) stays sharp while the world
focuses far: an overlay camera's pixels are outside the world's focus. Only opaque
and cutout surfaces write that depth: a transparent viewmodel pixel (a muzzle flash,
scope glass) takes the blur of what lies behind it, while opaque viewmodel meshes
stay sharp. The quality tier sets its cost:
off on **Low**, blurred at half resolution on **Medium**, full on **High**; the
player's `depth_of_field` option turns it off, and the editor's Scene view never runs
it. While it runs the view writes the velocity target its depths come from, as for
motion blur; mode `Off` costs nothing and draws exactly what no depth of field does.
The Inspector's **Depth of Field** section edits the same fields, and they are saved
with the scene. `Debug.Stats().gpu_passes.depth_of_field` times it.

Tween paths: `Graphics.depth_of_field.gaussian_start`, `Graphics.depth_of_field.gaussian_end`, `Graphics.depth_of_field.gaussian_max_radius`, `Graphics.depth_of_field.focus_distance`, `Graphics.depth_of_field.aperture`, `Graphics.depth_of_field.focal_length`.

<!-- effect: motion_blur -->
#### Motion blur

**Motion vectors** (#1435). A view whose base camera runs motion blur (it is on, and
the tier is Medium or High) also writes a per-pixel velocity buffer from its opaque
pass: each solid's motion since last frame, through its own last-frame transform and
skin, so an enemy running past a still camera moves in it and a viewmodel camera's
gun, turning with its camera, does not. Mesh particles ride the opaque pass and write
the camera's motion alone; transparent surfaces, sprite particles, ribbons, decals and
UI write none; the sky writes the camera's rotation. A first draw, a scene load, Play
or Stop, a switch of the main camera or a resize has no last frame, so nothing smears
across a cut; neither does a camera newly in the stack (a weapon switch's viewmodel
camera). No other frame pays for it; `Debug.Stats().velocity_passes` counts the passes
that wrote it.

**Per-object motion blur** (#1436) reads that buffer, so what blurs is what moved on
screen: an enemy running past a still camera, a swing, a ragdoll, and the world under
a turning camera, while a viewmodel camera's gun stays sharp. It is the
reconstruction filter shooters use (McGuire 2012, as refined for CoD: Advanced
Warfare): each pixel blurs along the longest motion near it, nearer surfaces smear
over farther ones and never the reverse, so a moving silhouette's blur reaches past
its edge onto the background behind it. A pixel's *streak* is its motion over one
frame times the camera's **intensity** (Unity's *Intensity*: the shutter fraction,
`0.5` a 180° shutter), at most the camera's **clamp** long (Unity's *Clamp*: a fraction
of the view's height), so a fast flick never smears the whole screen. **Samples** is
how many taps each pixel reads along it. A frame where nothing moved is
pixel-identical to motion blur off, and a frame with no motion history (the first, a
cut) blurs nothing. Like FXAA it is the camera's: it runs without a post-processing
volume. The editor's Scene view never blurs and writes no velocity, as Unity's
doesn't; the Game view and the player do. Depth ordering holds under a `DepthOnly`
overlay camera: each camera writes which camera of the stack drew a surface and its
depth along that camera's view axis beside its motion. A higher camera is in front whatever the
depths say, so a viewmodel camera's gun stays sharp even against a wall nearer to
the world camera than the gun is to its own; within one camera the nearer surface is
in front, so an enemy passing a wall smears over the wall at full strength. A surface
that was behind the camera last frame (a projectile past the player's head) writes no
motion. A render-texture camera drawn every N frames (`update_every`) blurs one
frame's worth of motion, not N. Bloom reads the blurred image, as in URP and HDRP. Its
cost is `Debug.Stats().gpu_passes.motion_blur`.

<!-- effect: exposure -->
#### Exposure

**Exposure** (#1444) is a plain HDR scale ahead of the grade, the image times
`2^ev`: in `"Fixed"` mode the volume's EV (`Graphics.SetExposure`), in `"Automatic"`
the metered one. A new volume starts `Fixed` at `0`.

**Automatic exposure** (#1444) is Unity HDRP's *Automatic Histogram*: eye
adaptation, so stepping from a dark corridor into daylight blinds you for a moment and
a dark room becomes readable once you're in it. With the mode `"Automatic"`, each frame
the renderer meters its view's HDR scene into a 64-bin log-luminance histogram (a fixed
64 × 64 sample, on the GPU), averages the bins between the **low** and **high
percentiles** (the darkest and brightest pixels ignored), and computes the exposure
that brings that average to middle grey, plus the **compensation**, held to the
**limits**. The applied exposure eases towards it at **speed up** EV per second while
rising (the image brightening: into the dark) and **speed down** while falling (into
light). Every EV is the unit of the fixed `exposure` (the image is scaled by `2^ev`), so
a dark room meters a positive EV, daylight a negative one, and the limits bound the
exposure itself. Adaptation runs on **game time**: a paused game (`Time.timeScale = 0`)
holds its exposure, and a headless capture at tick N is reproducible. Outside Play it
snaps to the metered value every frame, so the editor shows the settled exposure; in
Play it snaps on a cut (the first frame, a scene load, Play or Stop, a switch of the main
camera, a resize). Each view meters itself: a render-texture camera adapts to its own
picture, never the window's. The exposure is applied before the grading LUT, like the
fixed one. `Graphics.GetExposure` returns the configured `exposure` in both modes,
never the adapted one: scripts run in the deterministic sim, and the adapted EV is read
back from the GPU (another GPU may meter a hair differently). An agent checking
adaptation reads it with the dev-only `Debug.GetAdaptedExposure()`, a frame or so late
(exactly, in a headless capture). There is no player
option for it; the exposure is the developer's look. The Inspector's **Color
Correction** section edits the same fields, and they are saved with the scene. A new
volume starts `Fixed`.

Tween paths: `Graphics.exposure.fixed_ev`, `Graphics.exposure.automatic.min_ev`, `Graphics.exposure.automatic.max_ev`, `Graphics.exposure.automatic.compensation`, `Graphics.exposure.automatic.speed_up`, `Graphics.exposure.automatic.speed_down`, `Graphics.exposure.automatic.low_percent`, `Graphics.exposure.automatic.high_percent`.

#### Lens effects

**Lens effects** (#1439) are Unity's **Vignette**, **Chromatic Aberration** and
**Film Grain** overrides, built into the volume (they were authored `Shader.Bake`
blocks before). Each is off until its `active` flag is set; its strengths then start
at a visible `0.3` (Unity's start at `0`). They run inside the composite, in Unity's
order: **chromatic aberration** splits green and blue outwards along the radius from
the screen centre, three taps on the HDR image before the grade, growing with the
squared distance so the centre stays clean; the **vignette** fades the graded image
towards its `color` with the distance from its `center`; **film grain** is last,
before the custom effects and FXAA: per-pixel noise scaling the colour, faded in the
highlights by `response`. The grain is seeded by the pixel and the game clock
(`Scene` shader time), so a capture at a given tick is reproducible and an
edit-mode capture (time `0`) is stable. An effect off (or at intensity `0`) costs
nothing and leaves the image byte-identical. The player's `vignette`,
`chromatic_aberration` and `film_grain` options gate them (*Player options*), and
the Inspector's **Lens Effects** section edits the same fields.

<!-- effect: chromatic_aberration -->
#### Chromatic aberration

Splits green and blue outwards from the screen centre, on the HDR image before the
grade; its **intensity** (`0`–`1`) is how far the channels split at the corners. The
player's `chromatic_aberration` option gates it. Tween paths: `Graphics.chromatic_aberration.intensity`.

<!-- effect: ssr -->
#### Screen-space reflections

**SSR** marches each surface pixel's reflected ray against the depth buffer and adds
what it hits (the sky where it hits nothing), weighted towards grazing angles, on the
HDR image before bloom. The real screen-space pass runs on the **High** tier only; on
Low and Medium the forward shader's cubemap reflection stands in, its quality and
temporal upsampling taken from the same settings. The player's `ssr` option gates it.
A new volume starts with SSR on at `"High"` quality. No tween paths: its settings are
a switch and a name.

<!-- effect: bloom -->
#### Bloom

**Bloom** bleeds the scene's bright parts into their surroundings: a bright pass
keeps the light above the **threshold** (by luminance), a gaussian blur spreads it at
half the view's resolution (a quarter on Low), and the composite adds it back times
the **intensity**, before the exposure. It reads the scene after depth of field and
motion blur, so a moving highlight blooms along its streak. The player's `bloom`
option gates it. A new volume starts with bloom on at intensity `1`, threshold `0.8`.
Tween paths: `Graphics.bloom.intensity`, `Graphics.bloom.threshold`.

<!-- effect: lens_flare -->
#### Lens flare

**Lens flare** (#1443) is Unity URP's *Screen Space Lens Flare*: built from bloom's
blurred bright pass, so **every** bright pixel (the sun, a muzzle flash, a fire)
flares with no per-light authoring. Each bright pixel throws **ghosts** mirrored
through the screen centre (`ghost_count` of them, each at its own scale and lens
tint, so a light left of the centre ghosts on the right), a **halo** ring around the
centre, lit most by what is near it, and optional horizontal **streaks**, the
anamorphic lens's, the same share of the screen at every resolution. The whole is multiplied by the `tint` and the `intensity`, its red
and blue split by its own `chromatic_aberration`. It renders at half bloom's
resolution (a quarter of the view's) and is added onto the HDR scene after bloom's
bright pass, so the exposure and the grade apply to it and it never blooms itself.
**It runs only while bloom runs**, since it reads bloom's chain: bloom off, or the
player's `bloom` option off, means no flare. It is off on the Low tier, and the
player's `lens_flare` option gates it. A new volume starts with it off; once on, four
ghosts, a halo at `0.5` and no streaks. A frame with nothing above bloom's threshold
is byte-identical with it on and off, and a flare that can add nothing (no ghosts,
halo or streaks, or a black tint) costs no pass. Its GPU time is
`Debug.Stats().gpu_passes.lens_flare`. Tween paths: `Graphics.lens_flare.intensity`,
`Graphics.lens_flare.ghost_intensity`, `Graphics.lens_flare.halo_intensity`,
`Graphics.lens_flare.streak_intensity`, `Graphics.lens_flare.chromatic_aberration`.

#### Colour grading

**Colour grading** (#1440) is Unity URP's: **white balance** (temperature warms
towards yellow or cools towards blue, tint pushes towards magenta or green),
**Lift / Gamma / Gain** (three trackballs moving the blacks, the midtones and the
whites: the colour's hue tints that range, a grey colour tints nothing, and the
offset brightens or darkens it), a **hue shift** rotating every hue, and contrast
and saturation. Exposure stays a plain HDR scale ahead of all of it. White balance,
Lift / Gamma / Gain and the hue shift act on the scene's linear HDR colour, before
the tonemap; contrast, saturation and the display gamma act on the tonemapped
colour. The renderer bakes the whole grade and the tonemap into a 32³ lookup table
per view and grades each pixel with one fetch from it; the table is rebaked only on
a frame where one of those knobs or the tonemap changed, which `Debug.Stats()`
counts as `grade_lut_bakes`. Each view grades with its base camera's volume, like
the rest of its post-FX. The Inspector's **Color Correction** section edits the
same fields, and they are saved with the scene.

<!-- effect: white_balance -->
#### White balance

Temperature and tint, each `-100`–`100`, neutral `0, 0`:
`Graphics.SetWhiteBalance(temperature, tint)`. Tween paths: `Graphics.white_balance.temperature`, `Graphics.white_balance.tint`.

<!-- effect: lift_gamma_gain -->
#### Lift / Gamma / Gain

Three trackballs, each a colour and an offset (`Graphics.Set{Lift,Gamma,Gain}Wheel`).
The offsets tween: `Graphics.lift_gamma_gain.lift.offset`, `Graphics.lift_gamma_gain.gamma.offset`, `Graphics.lift_gamma_gain.gain.offset`.

<!-- effect: color_adjustments -->
#### Color adjustments

Contrast and saturation (neutral `1`) on the tonemapped colour, and the hue shift
(`-180`–`180` degrees, neutral `0`) before the tonemap: Unity's *Color Adjustments*.
Tween paths: `Graphics.color_adjustments.contrast`, `Graphics.color_adjustments.saturation`, `Graphics.color_adjustments.hue_shift`.

<!-- effect: tonemapping -->
#### Tonemapping

The operator (`"ACES"`, the default, `"Reinhard"` or `"None"`) that brings HDR to the
display, and the display-gamma tweak after the rest of the grade.

**Gamma** is a display-gamma *tweak*, not the output encode: the render target
(window, Game view, and headless screenshot alike) is sRGB and encodes linear →
display in hardware, exactly once (#415). `1.0` leaves the image untouched; `> 1`
lifts the midtones, `< 1` darkens them.

Tween paths: `Graphics.tonemapping.display_gamma`.

<!-- effect: color_lookup -->
#### Color Lookup

**Color Lookup** (#1441) is Unity's override of that name: a LUT a colourist
graded in Photoshop, DaVinci Resolve or Affinity and exported, applied last, on the
display colour after the tonemap and the rest of the grade, sRGB-encoded (as URP
applies it), and blended in by its **contribution** (`0` none, `1` all of it). Two
formats are read:

- **`.cube`** (Adobe's Cube LUT format, what Resolve and Photoshop export): a 3D
  table, `LUT_3D_SIZE` from `2` to `65` (`33` and `65` are Resolve's), with its
  `DOMAIN_MIN` / `DOMAIN_MAX` (or Resolve's `LUT_3D_INPUT_RANGE`), `#` comment lines
  and `TITLE`. A 1D LUT is refused.
- **A strip `.png`**, Unity's 2D layout, common in asset packs: `N` slices of
  `N × N` side by side, `N² × N` pixels (`256×16`, `1024×32`), blue by slice, red
  to the right and green upwards (the bottom row is green `0`).

Put the file under the project's **`assets/`** (`assets/luts/` is a good home) and
name it by its project-relative path, as a texture is named: `Graphics.SetColorLookup(
"assets/luts/warm.cube")`. Only there is it reloaded when an outside tool saves over
it (the editor watches `assets/`) and copied into an exported game; a file elsewhere
loads, but neither follows. The path is stored as written with `/` separators and
no leading `./`, so one file is one entry. The renderer folds it into the view's grading LUT, so it
costs a pixel nothing, and rebakes when the path, the contribution or the file
changes (a file saved over is read again). A missing or malformed LUT is logged
once, naming the file (and, for a `.cube`, the line), and grades as if none were
set. The Inspector's **Color Lookup** rows, at the end of **Color Correction**, edit
the same fields; the path is written when its field loses focus.

Tween paths: `Graphics.color_lookup.contribution`.

<!-- effect: vignette -->
#### Vignette

Fades the graded image towards its **colour** with the distance from its **centre**;
**intensity** is how strongly, **smoothness** how soft the falloff. The player's
`vignette` option gates it. Tween paths: `Graphics.vignette.intensity`, `Graphics.vignette.smoothness`, `Graphics.vignette.center.x`, `Graphics.vignette.center.y`.

<!-- effect: film_grain -->
#### Film grain

Per-pixel noise on the graded image, the last thing the composite draws, faded in
the highlights by its **response**; seeded by the game clock, so a capture at a tick
is reproducible. The player's `film_grain` option gates it. Tween paths: `Graphics.film_grain.intensity`, `Graphics.film_grain.response`.

<!-- effect: custom_effects -->
#### Custom effects

**Custom effects** (#397) are authored postfx modules — `Shader.Bake` with
`pass = "postfx"` (`Shader.md`) — run by the post chain, the way Unity's volume
stack runs custom post effects: damage vignettes, a low-health grayscale, CRT
scanlines on a menu camera.

```lua no-run
Shader.Bake({ pass = "postfx", name = "hurt",
  blocks = { { id = "damage_vignette", params = { color = {0.8, 0.0, 0.0} } },
             { id = "tint", params = { color = {1.0, 0.6, 0.6} } } } })
Graphics.SetCustomEffects({ "hurt" })   -- next frame: red-edged, reddened
Graphics.SetCustomEffects({})           -- off again
```

- **Order and placement.** The list runs in order, **after tonemapping and before
  FXAA**: on the display-referred image in `[0, 1]` that grades like these are
  designed for, and before anti-aliasing so an effect's own hard edges (scanlines,
  posterize bands) are smoothed like the scene's. Each entry is one fullscreen pass;
  names may repeat.
- **Names** are what `Shader.Bake` was given (the file is `<name>.wgsl` in the
  default authored-shader dir, `assets/shaders`). `SetCustomEffects` raises
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

```lua no-run
Shader.Bake({ pass = "postfx", name = "hurt",
  blocks = { { id = "damage_vignette", params = { color = {0.8, 0, 0} } } } })
Graphics.SetCustomEffects({ "hurt" })
function Update()
  Graphics.SetPostParam("damage_vignette.intensity", 1 - health / maxHealth)
end
```

- **Where they live.** On the active volume (`custom_effects.params`, saved with
  the scene like its other knobs), Unity's volume-profile overrides without the
  blending. A set is a uniform write the next frame packs — never a re-bake.
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
- **Tweening.** `Tween.To(volume, "Graphics.<name>", …)` eases a param of the
  volume entity `volume` over time (#661) — a damage vignette fading back to 0 —
  through the same check as `SetPostParam`; see [`Tween.md`](Tween.md).

<!-- effect: fxaa -->
#### FXAA

**FXAA** is the anti-aliasing pass at the very end of the chain, running on the
tonemapped image just before it reaches the screen. It is **on by default** — a
scene saved before the knob existed loads with it on, and so does a scene with no
camera component at all, which is why `GetFxaaActive` returns `true` rather than
`false` when there is no active camera. It is *not* gated by the quality preset (the player's `fxaa` option can turn it off):
it runs on every tier including Low, where a cheap fullscreen pass is exactly the
anti-aliasing an integrated GPU wants. Turning it off restores raw, stair-stepped
edges — useful for a pixel-art look, or for diffing screenshots.

### Lighting settings

The volume also carries the sun's shadow cascades and the ambient occlusion, which
shape the lighting passes rather than the post chain, and the scene its fog.

**Sun shadows** are cascaded (#435): the camera's view, out to the **shadow
distance**, is split into `1`–`4` slices, each with its own shadow map fitted to it
and following the camera, so the first few metres get the sharpest shadow and a
caster anywhere in a level still casts. Past the distance shadows fade out over its
last tenth. Both knobs live on the main camera's post-processing component (the
Inspector's **Shadows** section edits the same fields; #1434) and are saved with the
scene, so an indoor level can spend its cascades on 40 m and an outdoor one on 150 m;
without an active volume on the main camera the defaults above apply. The cascades
follow the screen, and render-texture cameras sample the same maps. Each cascade costs one more depth pass for
the moving casters (static casters are re-drawn only when a cascade moves, #355).

**Fog** (#437) is a **scene setting**, not a volume knob: it is saved with the
scene, edited in the Inspector's **Scene Settings → Fog** section (the same fields),
and the `Fog*` functions work whether or not the scene has a post-processing
volume. One shared shader function applies it to **every world-space pass** —
opaque and transparent surfaces, unlit surfaces, alpha-blended and additive
particles — and decals are part of the surface they mark (#638), so smoke and
bullet holes fade exactly like the wall behind them, and the sky blends to the fog colour at the horizon (fading out toward the
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
exaggerated). The knobs live on the post-processing component (the Inspector's
**Ambient Occlusion** section edits the same fields) and are saved with the scene;
scenes saved before the knob existed load with it on. Each view runs its base
camera's AO, like the rest of its post-FX (*Post-FX is per view*, above); a camera
without an active volume runs none.

Its cost is gated by the quality preset: **off on Low**; on **Medium** 8 samples
per texel at half resolution; on **High** 16 at full resolution. Either way each
camera also draws its visible solids once more, depth only (a prepass), and runs a
25-tap blur at full resolution. The frame stats report it (`ssao_samples`, and the
prepass inside `draw_calls` / `triangles`; see `Debug`). `SetSsaoActive(false)` —
or intensity `0` — skips every one of those passes.

The global **quality preset** gates the heavier passes (SSR is High-tier only;
motion blur, SSAO, depth of field and the lens flare are off on Low, SSAO and depth of field are
half-resolution on Medium; FXAA runs on all of them). `SetQuality` writes a shared
resource cell that the platform layer reads each frame and hands to the renderer,
which reallocates its bloom buffers when the tier actually changes. Unrecognized
tonemap/quality names are ignored (the current value is kept).

### Player options

The **player graphics options** (#1438) are what an options menu's "Motion blur:
off" writes. They **gate** effects rather than set them: an effect runs for a view
only when its camera has it on **and** the player's option allows it **and** the
quality tier runs it. So the developer's authored look stays on each camera, and the
player's choice holds for every camera and every scene, including ones loaded later.
An option can only turn an effect off; it never turns on what a camera left off.

<!-- sorted: one row per option, by name, the order `GetOptionNames` lists them in -->
| Option | Gates |
|---|---|
| `bloom` | the camera's volume's bloom |
| `chromatic_aberration` | the volume's chromatic aberration |
| `depth_of_field` | the volume's depth of field (off on Low) |
| `film_grain` | the volume's film grain |
| `fxaa` | the camera's FXAA |
| `lens_flare` | the volume's lens flare (off on Low; only while bloom runs) |
| `motion_blur` | the camera's motion blur (also off on Low) |
| `ssao` | the volume's ambient occlusion (off on Low) |
| `ssr` | the volume's screen-space reflections (High only) |
| `vignette` | the volume's vignette |

Names are snake_case; an unknown name raises an error listing the valid ones.
`SetOption` takes effect next frame and is **persisted** through `Storage`
(`graphics.options`, beside `graphics.quality`) at the Stop and quit boundaries, so
the choice sticks across relaunches. Only the options the player set are saved
(Unity's PlayerPrefs), so the rest keep following the project defaults, even a
default a later version of the game changes.

The editor's **Scene view** ignores the player options, as Unity's Scene view ignores
PlayerPrefs: it gates on the camera and the tier only. The Game view, the player and
every headless capture (a `play` scenario's screenshot, an MCP capture) honour all
three, so a shot shows what `GetOption` says.

**Project defaults.** What a fresh install starts with is the game's, set once for
the whole project: `SetDefaultOption`, or the checkboxes under **File → Build
Settings → Player graphics defaults**, which write the same value into
`project.rusty` (`"graphics_options"`; see [`Application.md`](Application.md)). At
startup the saved player options are read over these defaults, so an option the
player never touched follows the project, and changing a default applies at once to
every option the player hasn't set (in the editor, a preview of a fresh install).
`ResetOptions` is the menu's "Restore defaults": it forgets the player's choices. A game that doesn't want players choosing simply leaves the switch out
of its menu: every option defaults to allowed.

```lua
-- An options menu: one toggle per switch, and a "Restore defaults" button.
for _, name in ipairs(Graphics.GetOptionNames()) do
  local on = Graphics.GetOption(name)
  -- draw a toggle labelled `name`, showing `on`; when the player flips it:
  -- Graphics.SetOption(name, not on)
end
-- on "Restore defaults":
Graphics.ResetOptions()
```

**Determinism.** Every `Graphics` write is **one-way** into render-only state: the
post-FX volume and the preset and options cell are read by the render layer, never by
`FixedUpdate`. Toggling these knobs therefore cannot change how the deterministic
sim evolves, and a headless replay stays bit-for-bit stable regardless of them.

The quality preset and the `Video` settings below are **persisted** through
`Storage` (the `graphics.quality` key and the `video` namespace): the windowed app
reads them at startup and writes them back, the tier at the Stop and quit
boundaries and the video settings at quit, so the app relaunches at the
last-chosen tier and video settings.
