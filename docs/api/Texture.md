## `Texture`

Agent-composed **procedural texture authoring** (#270): build a *recipe* — a small
DAG of curated ops — and **bake** it to a tiling `.png` a material map slot consumes.
This is the texture leg of the authoring layer and the shared spine (recipe →
op-runner → bake) the material/shader legs build on.

A recipe is authored as a Lua **table** whose shape mirrors the on-disk JSON document
one-to-one, so the same DAG describes a Lua-built recipe and one loaded from a file.
Output maps are **seamless by construction** (the procedural domain wraps) and
**deterministic** — the same recipe + `seed` always bakes a byte-identical PNG (every
stochastic op draws from a seeded integer hash, never wall-clock or unseeded RNG).

| Function | Signature | Returns |
|---|---|---|
| `Texture.Bake` | `(recipe, path, slot)` | the written `path` |
| `Texture.BakeJson` | `(json, path, slot)` | the written `path` |
| `Texture.ToJson` | `(recipe)` | the recipe's canonical JSON string |

`recipe` is a table; `json` is its serialized form (from `Texture.ToJson`, or a saved
`.json`). `slot` names the target map and selects the **glTF encoding** applied on the
way out (case-insensitive, `-`/`_` ignored):

| `slot` | Encoding | Channel packing |
|---|---|---|
| `base_color` / `albedo` | **sRGB** | — |
| `emissive` | **sRGB** | — |
- `normal` — linear | tangent-space (use a `bump_to_normal` op)
| `roughness` | linear | — |
| `metallic` | linear | — |
| `metallic_roughness` / `orm` | linear | **metallic → B, roughness → G** (one-shot packed MR) |
| `data` | linear | — (height/masks/anything raw; the safe default) |

The returned `path` drops straight into a material slot, e.g.
`Material.SetTexture(id, Texture.Bake(recipe, "out/albedo.png", "base_color"))`.

### The recipe shape

```lua
{
  resolution = 1024,        -- power-of-two canvas; sane default 1024
  seed = 42,                -- folded into every stochastic op (default 0)
  output = "n1",            -- output node id (optional; defaults to the last node)
  nodes = {
    { id = "n0", op = "noise", kind = "fbm", scale = 8.0, octaves = 4 },
    { id = "n1", op = "color_ramp", inputs = {"n0"},
      stops = { { pos = 0.0, color = {0.1,0.05,0.0,1} },
                { pos = 1.0, color = {0.6,0.4,0.2,1} } } },
  },
}
```

Each node has a stable `id`, an `op` tag, the op's params as sibling fields, and an
optional `inputs` array of upstream node ids (in order). The runner evaluates the DAG
in dependency order in linear `[f32]` RGBA, then the bake encodes/packs per `slot`.

### The op-set (curated "Blender-lite")

Grouped like Blender's node menus; `op` is the tag string in each node.

- **Generators** (no inputs): `constant {color}`; `noise {kind="perlin"|"fbm", scale,
  octaves}`; `voronoi {scale, output="distance"|"cells"}`; `gradient
  {kind="linear"|"radial"}`; `wave {kind="bands"|"rings", frequency}`; `brick {rows,
  cols, mortar}`; `checker {tiles, color_a, color_b}`; `white_noise`.
- **Color** (1 input, 2 for `mix`): `color_ramp {stops}`; `mix {mode, factor}` with
  `mode` ∈ `mix/add/multiply/screen/overlay/subtract/difference`; `invert`;
  `bright_contrast {bright, contrast}`; `hue_sat_value {hue, sat, value}`; `gamma
  {gamma}`.
- **Vector / normal**: `mapping {scale=[x,y], rotation, translation=[x,y]}`;
  `bump_to_normal {strength}` (height → tangent-space normal); `combine_rgb` (three
  grayscale inputs → RGB); `separate_rgb {channel=0..3}`.
- **Converter / math**: `math {func, value}` with `func` ∈
  `add/subtract/multiply/divide/power/min/max/abs/fract/sqrt`; `map_range {from_min,
  from_max, to_min, to_max}`; `clamp {min, max}`; `rgb_to_bw`.
- **Filter**: `blur {radius}` (separable box blur; wraps, so it stays tiling).

Example — a packed metallic-roughness map baked in one shot (red = metallic,
green = roughness, routed to B/G by the `metallic_roughness` slot):

```lua
Texture.Bake({
  resolution = 512, seed = 1,
  nodes = {
    { id = "m", op = "voronoi", scale = 12.0, output = "distance" }, -- metallic, R
    { id = "r", op = "noise", kind = "fbm", scale = 6.0, octaves = 3, inputs = {} }, -- roughness, G
    { id = "mr", op = "combine_rgb", inputs = {"m", "r"} },          -- R=metallic, G=roughness
  },
  output = "mr",
}, "out/crate_mr.png", "metallic_roughness")
```
