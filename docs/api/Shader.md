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
registered by name; a `ShaderRegistry` pointed at that dir loads it.

This is authoring shaders that **fit the engine**, not a general-purpose shader
compiler: the agent composes only from the curated block library — there is no
free-form WGSL synthesis — and the output targets the existing contract (no new
passes, bindings, or render features).

| Function | Signature | Returns |
|---|---|---|
| `Shader.Bake` | `(recipe [, out_dir])` | the written `<out_dir>/<name>.wgsl` path |
| `Shader.BakeJson` | `(json [, out_dir])` | the written path |
| `Shader.Validate` | `(recipe)` | array of the composed module's entry-point names (dry-run; **writes nothing**) |
| `Shader.ToJson` | `(recipe)` | the recipe's canonical JSON string |
| `Shader.Blocks` | `(pass)` | array of the curated block ids for `pass` (`"surface"` \| `"postfx"`) |

`recipe` is a table; `json` is its serialized form (from `Shader.ToJson`, or a saved
`.json`). `out_dir` defaults to `project/assets/shaders`. Use `Shader.Validate` to
compose-check a recipe before committing to a bake, and `Shader.Blocks` to discover
the catalog rather than guess block ids.

### The recipe shape

```lua
{
  pass = "postfx",          -- "surface" | "postfx" (the contract the output honours)
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
Unsupplied params fall back to the block's defaults. The assembler templates the
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
- **`postfx`** — a self-contained **fullscreen-triangle** fragment program over the
  HDR scene color (`vs_fullscreen` + `fs_main`), the most self-contained pass; each
  block grades the sampled color per-pixel.

### The block library (curated)

`pass` selects which catalog is valid; `op` is each block's `id`. Surface blocks fold
into the lit color; postfx blocks grade the sampled scene color.

- **Surface** (forward-pass fragment looks): `toon_ramp {steps}` (cel banding);
  `fresnel_rim {color, power, strength}` (view-dependent rim glow); `tint {color}`;
  `emissive_boost {color, strength}` (glow masked by the emissive map);
  `uv_scroll_stripes {frequency, strength}` (UV-driven banding); `desaturate
  {amount}`; `height_fog {color, top, bottom}` (world-height fog blend).
- **Postfx** (fullscreen grades over the HDR color): `tint {color}`; `exposure
  {stops}`; `saturation {amount}`; `grayscale`; `vignette {strength, radius}`;
  `scanline {count, strength}`; `posterize {levels}`; `contrast {amount}`.

Example — bake a stylized surface variant and load it by name:

```lua
Shader.Bake({
  pass = "surface", name = "enemy_toon",
  blocks = {
    { id = "toon_ramp", params = { steps = 3.0 } },
    { id = "fresnel_rim", params = { color = {1.0, 0.2, 0.1}, power = 4.0 } },
  },
})  -- → "project/assets/shaders/enemy_toon.wgsl" (validated, ready to load)
```

> **Faithfulness:** the read-site is the engine's `ShaderRegistry` — the same loader
> the shipped shaders use. The module is validated through that identical `naga_oil`
> compose path **at bake**, so a baked variant is, by construction, one the engine can
> load. See `docs/api-faithfulness.md`.
