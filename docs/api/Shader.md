## `Shader`

Agent-composed **WGSL shader authoring** (#272): compose a *recipe* — a base pass
plus a list of curated **building blocks** — and **bake** a `.wgsl` module that
conforms to the engine's existing pass + bind-group contract. This is the shader
leg of the authoring layer; it works in *text*, not pixels (cf. `Texture`).

The bake **validates the assembled module by composing it through `naga_oil`** — the
**same loader path the engine uses** (`ShaderRegistry`, with `common.wgsl`
registered) — and **rejects a module that won't compile, writing no file**. So a bad
shader is caught at authoring time and never ships. On success the module is written
to the authored-shader workspace (`project/assets/shaders/<name>.wgsl` by default),
registered by name. A **surface** module is then used by naming it on a material —
`Material.SetShader(id, "enemy_toon")` or a recipe's `shader` key (see `Material.md`);
re-baking it in a running session rebuilds it on the next frame. A **ui** module is
used by naming it on a UI graphic — `UI.SetShader(id, "hud_glitch")` (see
[*The `ui` pass*](#the-ui-pass)).

This is authoring shaders that **fit the engine**, not a general-purpose shader
compiler: the agent composes only from the curated block library — there is no
free-form WGSL synthesis — and the output targets the existing contract (no new
passes, bindings, or render features).

| Function | Signature | Returns |
|---|---|---|
| `Shader.Bake` | `(recipe [, out_dir])` | the written `<out_dir>/<name>.wgsl` path |
| `Shader.Validate` | `(recipe)` | array of the composed module's entry-point names (dry-run; **writes nothing**) |
| `Shader.ToJson` | `(recipe)` | the recipe's canonical JSON string |
| `Shader.Blocks` | `(pass)` | the curated blocks for `pass` (`"surface"` \| `"postfx"` \| `"ui"`): `{ {id, desc, params = { {name, default, arity, runtime} }, textures = {slot…}, stage }, … }` — `stage` is `"color"` or `"uv"` |

`recipe` is a table **or** its serialized JSON string (from `Shader.ToJson`, or a
saved `.json`) — both forms decode alike, with the same errors. `out_dir` defaults to `project/assets/shaders`. Use `Shader.Validate` to
compose-check a recipe before committing to a bake, and `Shader.Blocks` to discover
the catalog rather than guess block ids or params.

`Shader.Blocks` reads the same catalog the bake checks params against, so it is
always current (#411). Each block has its `id`, a one-line `desc`, and its `params`
in the order the block declares them: `name`, `default` (a number, or for a vector
param an array of `arity` numbers — the broadcast default, ready to paste), `arity`
(1 scalar, 2–4 vector) and `runtime` (whether the param can be changed per material
without a re-bake, #399). `textures` lists the extra texture slots the block
samples (`{"mask"}` for `dissolve`, empty for most) — the `shader_textures` keys a
material using it should name (#400).

```lua
for _, b in ipairs(Shader.Blocks("postfx")) do
  print(b.id, b.desc)
  for _, p in ipairs(b.params) do print("  ", p.name, p.arity, p.default) end
end
```

### The recipe shape

```lua
{
  pass = "postfx",          -- "surface" | "postfx" | "ui" (the contract the output honours)
  name = "warm_grade",      -- baked file becomes <name>.wgsl, registered by this name
  blocks = {                -- applied in order; each transforms the running color
    { id = "tint", params = { color = {1.0, 0.85, 0.6} } },
    { id = "vignette", params = { strength = 0.6, radius = 0.8 } },
    { id = "scanline" },    -- params optional; each block has sane defaults
  },
}
```

Each block has an `id` from the pass's catalog and an optional `params` map (a param
is a scalar like `0.6` or a small float array like `{1,0.5,0.25}` for a color).
Unsupplied params fall back to the block's defaults.

**Params are checked, not guessed.** A param the block doesn't declare (`strenght`),
an array where a number is expected, or a color with the wrong number of components
is an **error** naming the block, its position (`blocks[i]`), the bad key and the
params the block declares. The one convenience: a single number for a vector param
broadcasts to every lane (`color = 0.5` is mid-gray). Unknown keys on the recipe or a
block entry (`param` for `params`) are errors too.

**A block may appear more than once.** Each entry is its own instance with its own
params — two tints at different points in the chain, two `fresnel_rim`s in different
colors, or a second fog band all compose. (In the assembled WGSL each instance's
params are constants named `<id>_<index>_<param>`, `index` being its position in
`blocks`, passed into the block's helper function, which is emitted once.) The assembler templates the
blocks into a complete module **deterministically** — the same recipe always
assembles byte-identical WGSL.

### Pass kinds & the contract

- **`surface`** — varies the *fragment look* of the forward/surface pass. The
  standard `vs_main` + the full PBR lighting are kept **verbatim** (so the variant
  binds against the forward pipeline unchanged — same `VertexInput`, the
  `LightingUniforms`/`EntityUniforms`/group(2) material maps, and `vs_main`/`fs_main`
  entry points); each block restyles the shaded color before the final write.
  Group 1 is the instanced layout (#470): the per-draw `EntityUniforms` plus a
  per-instance `instances` array (world matrix, probe SH). A surface variant baked
  before #470 was written against the old per-entity layout — re-bake it.
  Surface decals (#638) are folded into `fs_main`'s material inputs before the
  lighting, so a variant carries them like the standard shader. A surface variant
  baked before #638 carries the old `fs_main` and **shows no decals** until it is
  re-baked (it still draws correctly otherwise).
- **`postfx`** — a self-contained **fullscreen-triangle** fragment program over the
  **tonemapped** scene color (`vs_fullscreen` + `fs_main`), the most self-contained
  pass; each block grades the sampled color, and sampling blocks may also tap
  the input at offsets. A baked postfx module runs
  in a game once a volume lists it: `Graphics.SetCustomEffects({"crt"})` (see
  `Graphics.md`, *Custom effects*). The renderer loads it from the default output
  dir, so bake it there (no `out_dir`).

- **`ui`** (#427) — restyles **one UI graphic** (an `Image`, a `Shape` or a `Text`). The module
  is the engine's UI shader (`ui.wgsl`) kept verbatim — both entry-point pairs,
  `vs_main`/`fs_main` (screen canvases) and `vs_world`/`fs_world` (world canvases),
  with their sRGB handling, SDF text, fog, RectMask clip and Mask coverage — except
  `graphic()`, the function both fragment entry points take a graphic's colour from,
  which folds the blocks over it (clips and masks apply after it). It adds one
  binding: group 3, the per-graphic uniform (rect, clock, runtime params). See [*The `ui` pass*](#the-ui-pass).

### Shader inputs

What a block's helper can read (#398). Surface blocks get the forward contract:

- `in` — the interpolated `VertexOutput`: `world_position`, `world_normal`,
  `tex_coords`.
- `camera` — the `CameraUniforms`: `view_proj`, `camera_pos`, `fog`, and
  **`camera.time`**, the sim's game time in seconds (the same clock as Lua's
  `Time.time`). It is scaled by `Time.SetTimeScale`, freezes while the game is paused,
  and is **0 in edit mode and `Debug.Preview`**, so previews stay pixel-comparable and
  a replay renders the same frames. Pulses, scrolling, flicker: drive them off this.
- `entity` — the per-draw `EntityUniforms`; the material maps (`t_diffuse`,
  `t_emissive`, … with `s_diffuse`).
- **`t_mask`** — the material's extra *mask* texture (#400), sampled with
  `s_diffuse`: whatever texture the material names under `shader_textures.mask`,
  **white** when it names none or the file is missing. Only blocks that list
  `mask` in their `textures` read it, and a module declares it only when one of
  its blocks does.

Postfx blocks see the sampled color `c`, the fragment `uv`, and **`game_time()`** —
the same game clock (it reads the post params' `camera_pos.w`, bound at binding 0).
**Sampling blocks** (#402) also read the module's input at other places:
`source_tap(uv)` samples it (clamped at the screen edge) and `source_texel()` is
one pixel's size in uv, for stepping by whole pixels. A sampling block reads the
module's **input** — what the chain handed this module — not the colour the blocks
before it in the same recipe produced; it adds the offset taps' *difference* from
the centre tap to the running colour. First in a recipe that is exactly the
classic effect; after a grade it layers the same fringe or detail on top. Put
sampling blocks first when that distinction matters.

### The block library (curated)

`pass` selects which catalog is valid; `op` is each block's `id`. Surface blocks fold
into the lit color; postfx blocks grade the sampled scene color.

- **Surface** (forward-pass fragment looks): `toon_ramp {steps}` (cel banding);
  `fresnel_rim {color*, power, strength*}` (view-dependent rim glow); `tint
  {color*}`; `emissive_boost {color*, strength*}` (glow masked by the emissive map);
  `uv_scroll_stripes {frequency, strength, speed}` (stripes scrolling along V at
  `speed` stripes per game second); `desaturate
  {amount*}`; `height_fog {color, top, bottom}` (world-height fog blend);
  `hit_flash {color*, amount*}` (blend toward a flash color; `amount` 0 = off, 1 =
  solid, default 0 — hit feedback driven from a script);
  `pulse_glow {color* = 1, speed = 1, strength* = 1}` (emissive that breathes
  between 0 and `strength`, `speed` pulses per game second — pickups, objectives);
  `hologram {color* = 1, line_freq = 20, flicker = 0.15}` (scanlines along world
  height, `line_freq` per unit, rolling with game time, plus a Fresnel rim and a
  stepped flicker, in `color` over a faint copy of the surface — set `color`, e.g.
  `{0.3, 0.8, 1}`; pair it with a Transparent material for see-through);
  `uv_scroll {speed = 0.25}` (a **uv-stage** block: offsets the UVs by `speed`
  — a 2-vector, uv per game second — *before* any map is sampled, so the base,
  normal, metallic/roughness and mask maps all move together: conveyors, flowing
  energy, water. Its place in the recipe doesn't matter; every color block sees the
  moved UVs).
  Sampling the material's **mask** texture (see *Extra textures* below):
  `dissolve {amount*, edge_width = 0.05, edge_color* = 1, tiling = 1}` (fragments
  whose mask is below `amount` are cut away, a band `edge_width` wide above it
  glows `edge_color`; `amount` 0 = whole, 1 = gone — dissolve-on-death);
  `detail_overlay {tiling = 4, strength* = 1, scroll = 0}` (the mask tiled
  `tiling` times as an overlay: 0.5 gray is neutral, darker darkens, lighter
  brightens; `scroll` is uv per game second, so a non-zero `scroll` is a moving
  energy pattern); `triplanar_detail {scale = 0.5, strength* = 1}` (the mask
  projected in **world space** from the three axes and blended by the normal, so
  it tiles evenly across level geometry with no UV stretching or seams; `scale`
  is tiles per world unit, 0.5 gray neutral like `detail_overlay`).
  Params marked `*` are **runtime**: see below.
  Every surface variant is also fogged by the **scene fog** (`Graphics.SetFog*`,
  #437), applied after the blocks; `height_fog` is a per-material *look* layered
  under it (a glowing floor mist on one material), not a substitute for scene fog.
- **Postfx** (fullscreen grades over the tonemapped color, `[0, 1]`): `tint
  {color}`; `grayscale`; `vignette {strength, radius}`; `scanline {count,
  strength}`; `posterize {levels}`. Animated by game time: `film_grain
  {strength = 0.06}` (per-pixel noise, new every frame while time runs; frozen at
  time 0 in edit mode); `damage_vignette {color = 1, intensity = 0.5, pulse_speed
  = 0}` (edges blend toward `color` — set it, e.g. `{0.8, 0, 0}` for damage red;
  `pulse_speed` is radians per game second, 0 holds it steady). Sampling:
  `chromatic_aberration {strength = 0.01}` (R and B split radially from the
  centre, `strength` as a fraction of the distance to it); `sharpen {amount =
  0.5}` (5-tap unsharp mask); `radial_blur {strength = 0.05, center = 0.5}`
  (8 taps toward `center` in uv — speed/impact streaks). Params marked `*` are
  **runtime**, driven per frame on the volume with `Graphics.SetPostParam` (see
  below): `vignette.strength*`, `scanline.strength*`, `film_grain.strength*`,
  `damage_vignette.color*` and `.intensity*`, `chromatic_aberration.strength*`,
  `sharpen.amount*`, `radial_blur.strength*`; the rest are baked. Exposure, saturation and contrast are **not**
  blocks: the volume already grades them in HDR (`Graphics.SetExposure`,
  `SetSaturation`, `SetContrast`), so a recipe naming them is refused as an unknown
  block (#397).

### The `ui` pass

Bake with `pass = "ui"`, then `UI.SetShader(id, name)` (see `UI.md`); the model —
batching, the clock, fallback — is in [`docs/ui.md`](../ui.md#custom-shaders). Every
ui block's helper is
`fn ui_<id>(c: vec4<f32>, in: VertexOut, f: UiFrag, <params…>) -> vec4<f32>`, where:

- `c` — the graphic's colour so far, **premultiplied, display space** (what the UI
  pass blends): the texture × tint (or gradient), a Shape's SDF, or the SDF text with its outline and glow, with
  CanvasGroup alpha folded in. Colour beyond alpha *adds* light over what is under
  the graphic (the hologram's glow).
- `f.uv` — **rect-local**, `(0, 0)` at the graphic's bottom-left to `(1, 1)` at its
  top-right, following its rotation; `f.size` — the rect in pixels (target pixels
  on a screen canvas, reference units on a world one); `f.pixel` — the fragment's
  position in target pixels; `f.time` — the **UI clock**: unscaled seconds
  (`Time.unscaledTime`), so a paused menu (time scale 0) keeps animating; 0 in edit
  mode.
- Sampling blocks re-shade the graphic at a rect-local offset (`ui_shift`): the
  tap's difference from the centre is added to the running colour, and a tap off
  the rect is transparent — so a slice that jumps sideways leaves a gap.

The blocks (`*` = runtime, settable per graphic with `UI.SetShaderParam`):

- `scanlines {spacing = 4, strength* = 0.35, speed = 0}` — horizontal lines
  `spacing` px apart fade the graphic; `speed` scrolls them (px per second).
- `rgb_split {offset* = 3, angle = 0}` — red and blue re-sampled `offset` px apart
  along `angle` degrees: a cyan/yellow fringe at the edges.
- `glitch_slices {bands = 8, amount* = 12, density = 0.35, rate = 12}` — the rect cut
  into `bands` horizontal bands; a `density` fraction of them jumps sideways up to
  `amount` px, re-rolled `rate` times a second. `amount` 0 is off — drive it on a hit.
- `noise_flicker {strength* = 0.35, rate = 24}` — the whole graphic dims by up to
  `strength`, re-rolled `rate` times a second.
- `hologram {hue = 190, strength* = 1, spacing = 3, speed = 30, flicker = 0.12}` —
  luminance recoloured to `hue` degrees (190 cyan, 0 red, 120 green, 240 blue),
  glowing additively, with scan bands scrolling up at `speed` px/s and a flicker.
- `dissolve {amount* = 0, scale = 12, edge = 0.08, edge_color = 1}` — value noise
  (`scale` cells across the rect's height) eats the graphic: `amount` 0 whole, 1
  gone; the burning edge, `edge` wide, glows `edge_color`.
- `wipe {progress* = 1, angle = 0, softness = 0.05}` — reveal sweeping toward
  `angle` degrees (0: left → right, 90: bottom → top): `progress` 0 hidden, 1 shown.
- `radial_wipe {progress* = 1, start = 90, clockwise = 1, softness = 0.01}` — a
  clock sweep around the rect's centre from `start` degrees (90: twelve o'clock),
  clockwise unless `clockwise` is 0.

```lua
Shader.Bake({ pass = "ui", name = "holo_call",
              blocks = { { id = "hologram", params = { hue = 185 } }, { id = "scanlines" } } })
UI.SetShader(callScreen, "holo_call")
UI.SetShaderParam(callScreen, "hologram.strength", 0.6)
```

### Runtime params (surface, postfx)

A surface param marked **runtime** above is not baked as a constant: the shader
reads it at draw time, and `Material.SetShaderParam(id, "hit_flash.amount", 1)`
changes it for one entity (`Material.SetAssetShaderParam` for every entity sharing
the material) with no re-bake (see `Material.md`). Its recipe value is the default a
material starts from. A bake also writes `<name>.params.json` beside the module,
recording each runtime param's name and slot, so the engine resolves names without
reading WGSL. One shader holds at most 16 runtime params; more is a bake error.
A **postfx** shader's runtime params live on the post-processing volume instead
(#671): `Graphics.SetPostParam("damage_vignette.intensity", 0.7)` sets it for every
effect the active volume runs that has it (see `Graphics.md`); the module reads them
from its own group-1 uniform. A **ui** shader's
runtime params work the same way, per graphic: `UI.SetShaderParam(id,
"dissolve.amount", 0.5)` (see `UI.md`).

### Extra textures (surface)

Some surface effects need a *pattern* — a noise to dissolve along, a grime map
to overlay. A material names one per **slot** in `shader_textures` (see
`Material.md`); v1 has one slot, `mask`. Blocks that read it list it in
`Shader.Blocks`' `textures`. A material with no mask samples white, so
`dissolve` stays whole and `detail_overlay` brightens. Slot names are strict: an
unknown slot is an error naming it.

The natural producer is `Texture.Bake` (see `Texture.md`): bake a tiling noise and
point the mask at the PNG it wrote — or name an `rt:<name>` render texture. Bake
it with slot `data`: a mask holds values, not colour, so the renderer samples its
bytes raw (a mid-grey 128 reads ≈ 0.5), like every data map (#647). A render
texture reads as the linear values its camera shaded.

```lua
Shader.Bake({ pass = "surface", name = "enemy_die",
              blocks = { { id = "dissolve", params = { edge_color = {1, 0.4, 0.1} } } } })
Material.SetShader(enemy, "enemy_die")
Material.SetShaderTexture(enemy, "mask", "project/assets/textures/noise.png")
-- each frame while dying:
Material.SetShaderParam(enemy, "dissolve.amount", t)   -- 0 → 1
```

The cut reaches every pass that draws the mesh (#648): a dissolved fragment casts
no **shadow** and leaves no ambient occlusion, reading the same `amount` and mask
as the colour pass. At `amount = 1` the entity is invisible, shadow and all — no
need to hide it to finish the effect. Behind a UV-stage block (`uv_scroll`) the
cut samples the moved UVs in every pass.

Only a shader with a cutting block pays for this; every other shader keeps the
shared depth pipelines. So without one, `uv_scroll` moves the colour pass's UVs
only: a Cutout material's alpha-tested shadow keeps the unscrolled cut-out.

Example — bake a stylized surface variant and load it by name:

```lua
Shader.Bake({
  pass = "surface", name = "enemy_toon",
  blocks = {
    { id = "toon_ramp", params = { steps = 3.0 } },
    { id = "fresnel_rim", params = { color = {1.0, 0.2, 0.1}, power = 4.0 } },
  },
})  -- → "project/assets/shaders/enemy_toon.wgsl" (validated, ready to load)
Material.SetShader(enemy, "enemy_toon")  -- the enemy's material now renders with it
```

> **Faithfulness:** the read-site is the engine's `ShaderRegistry` — the same loader
> the shipped shaders use. The module is validated through that identical `naga_oil`
> compose path **at bake**, so a baked variant is, by construction, one the engine can
> load. See `docs/api-faithfulness.md`.
