## `Texture`

Agent-composed **procedural texture authoring** (#270): build a *recipe* — a small
DAG of curated ops — and **bake** it to a tiling `.png` a material map slot consumes.
This is the texture leg of the authoring layer and the shared spine (recipe →
op-runner → bake) the material/shader legs build on.

A recipe is authored as a Lua **table** whose shape mirrors the on-disk JSON document
one-to-one, so the same DAG describes a Lua-built recipe and one loaded from a file.
Output maps are **seamless by construction** (the procedural domain wraps, and every
periodic param is a whole count of repeats per tile — see *Tiling* below),
**resolution-independent** (`resolution` only chooses how finely the pattern is
sampled — see *Units* below) and **deterministic** — the same recipe + `seed` always
bakes a byte-identical PNG (every stochastic op draws from a seeded integer hash, never
wall-clock or unseeded RNG).

| Function | Signature | Returns |
|---|---|---|
| `Texture.Bake` | `(recipe, path, slot)` | the written `path` |
| `Texture.ToJson` | `(recipe)` | the recipe's canonical JSON string |
| `Texture.Ops` | `()` | the op catalog: `{ {op, category, inputs, params = { {name, type, default, required, values} } }, … }` |

`recipe` is a table **or** its serialized JSON string (from `Texture.ToJson`, or a
saved `.json`) — both forms decode alike, with the same errors. `slot` names the target map and selects the **glTF encoding** applied on the
way out (case-insensitive, `-`/`_` ignored). An unknown slot name is an **error** that
lists the valid ones — a typo like `"basecolour"` never silently bakes linear albedo:

| `slot` | Encoding | Channel packing |
|---|---|---|
| `base_color` / `albedo` | **sRGB** | — |
| `emissive` | **sRGB** | — |
| `normal` | linear | tangent-space (use a `bump_to_normal` op) |
| `roughness` | linear | — |
| `metallic` | linear | — |
| `metallic_roughness` / `orm` | linear | **metallic → B, roughness → G** (one-shot packed MR) |
| `data` | linear | — (height/masks/anything raw; the explicit way to get unencoded output) |

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
optional `inputs` array of upstream node ids (in order). **Unknown keys are errors**
everywhere in a recipe — on the recipe itself, on a node, on a ramp stop: `{ op =
"noise", scael = 8 }` is refused with a message naming `scael` and the params `noise`
takes, rather than baking with the default scale. The runner evaluates the DAG
in dependency order in linear `[f32]` RGBA, then the bake encodes/packs per `slot`.

### The op-set (curated "Blender-lite")

Grouped like Blender's node menus; `op` is the tag string in each node.

**Ask the engine, not this page.** `Texture.Ops()` returns the live catalog — the
list below is a summary of it. Each entry has the `op` tag, its `category`
(`generator` | `color` | `vector` | `math` | `filter`), how many `inputs` it consumes
(a missing input reads as a black canvas), and its `params`. Each param has a
`name`, a `type` (`number` | `integer` | `bool` | `color` (RGBA array) | `vec2` |
`enum` | `stops`), `required`, a `default` when it has one (absent when required),
and for an `enum` the accepted `values`. Units and rounding are the ones *Tiling*
and *Units* below describe. A unit test holds the catalog to the recipe parser, so
it can't name an op or param the parser doesn't take, or a default it doesn't apply.

```lua
for _, o in ipairs(Texture.Ops()) do
  if o.op == "brick" then
    for _, p in ipairs(o.params) do print(p.name, p.type, p.default) end
  end
end  -- rows number nil / cols number nil / mortar number 0.05
```

- **Generators** (no inputs): `constant {color}`; `noise
  {kind="perlin"|"fbm"|"ridged"|"turbulence", scale, octaves=1, lacunarity=2, gain=0.5}`
  (`ridged` is `1 − |n|` squared — rock veins, scratches; `turbulence` is `|n|` —
  smoke, grime; the fBM family stacks `octaves`, each `lacunarity`× the frequency and
  `gain`× the amplitude of the last; `perlin` is one octave); `voronoi {scale,
  output="distance"|"cells"|"f2"|"edges", randomness=1}` (`f2` is the distance to the
  second-nearest point, `edges` is F2 − F1 — ~0 on cell borders: cracks, dried mud,
  cobble outlines; `randomness` 0 is a regular grid, 1 full jitter); `gradient
  {kind="linear"|"linear_tiling"|"radial"}`; `wave {kind="bands"|"rings", frequency}`;
  `brick {rows, cols, mortar=0.05, output="mask"|"random"|"bevel"}` (`mortar` is the
  gap as a fraction of one brick, reading 0 in every output; `mask` is 1 on brick
  faces, `random` a seeded value per brick — feed a `color_ramp` for per-brick colour
  or roughness variation — and `bevel` a ramp from 0 at the mortar to 1 on the brick's
  centre line, a rounded-brick height for `bump_to_normal`); `shape
  {kind="circle"|"rect"|"rounded_rect"|"line", size=[w,h], roundness=0.25,
  softness=0}` (one shape centred in the tile, 1 inside and 0 outside: `circle` is the
  ellipse filling `size`, `line` a horizontal stroke `w` long and `h` thick with round
  caps — turn it with `mapping {rotation = 0.25}`; `roundness` 0–1 rounds a
  `rounded_rect`'s corners; `softness > 0` ramps the edge from 0 to 1 that far inside,
  a bevel profile for `bump_to_normal`); `checker {tiles, color_a, color_b}`;
  `white_noise`.
- **Color** (1 input, 2 for `mix`): `color_ramp {stops}`; `mix {mode, factor}` with
  `mode` ∈ `mix/add/multiply/screen/overlay/subtract/difference`; `invert`;
  `bright_contrast {bright, contrast}`; `hue_sat_value {hue, sat, value}`; `gamma
  {gamma}`.
- **Vector / normal**: `mapping {scale=[x,y], rotation, translation=[x,y],
  tiling=true}` (bilinear, wrapped resample; `rotation` in turns); `bump_to_normal
  {strength}` (height → tangent-space normal); `combine_rgb` (three grayscale inputs →
  RGB); `separate_rgb {channel=0..3}`; `warp {strength}` (2 inputs: samples input 1
  displaced by input 2's R/G, centred on 0.5 so mid-grey moves nothing; `strength` is in
  tile units; feed input 2 a `combine_rgb` of two different noises — a grayscale one has
  R = G and only pushes along the diagonal); `tile {count, jitter=0, rotation_jitter=0,
  scale_jitter=0}` (1 input: stamps the input once per cell of a `count`×`count` grid;
  each cell is seeded from the recipe `seed` with an offset of up to ±`jitter`/2 cells,
  a turn of up to ±`rotation_jitter`/2 turns and a scale in `1 ± scale_jitter/2`, each
  jitter clamped to 0–1. The input is read as **one stamp** — the unit square, nothing
  outside it — so give it empty margins (a `shape` smaller than the tile); a stamp
  pushed over its cell's border is drawn into the neighbour cell, and overlapping
  stamps keep the brighter value per channel. Rivets are `shape circle` → `tile`).
- **Converter / math**: `math {func, value}` with `func` ∈
  `add/subtract/multiply/divide/power/min/max/abs/fract/sqrt`; `map_range {from_min,
  from_max, to_min, to_max}`; `clamp {min, max}`; `rgb_to_bw`.
- **Filter**: `blur {radius}` (separable box blur; wraps, so it stays tiling).

### Tiling

Every periodic param is a **count of repeats per tile**, rounded at evaluation (the
field stays a number, so `4.5` is accepted — it bakes as `5`):

| Op | Param | Rounds to |
|---|---|---|
| noise, voronoi | `scale` | nearest whole number ≥ 1 |
| noise (fBM family) | each octave's `scale × lacunarity^octave` | nearest whole number ≥ 1 (so a fractional `lacunarity` still tiles) |
| wave | `frequency` | nearest whole number ≥ 1 |
| brick | `cols` | nearest whole number ≥ 1 |
| brick | `rows` | nearest **even** number ≥ 2 (the half-brick offset must close at the wrap) |
| tile | `count` | nearest whole number ≥ 1 |
| checker | `tiles` | up to **even** (an odd count puts two same-colour squares together at the wrap) |
| mapping (`tiling = true`) | `scale` | nearest whole number ≥ 1 per axis |
| mapping (`tiling = true`) | `rotation` | nearest quarter turn |

`mapping { tiling = false }` honours free scale and rotation — for decals and one-off
maps that are never tiled; it opens a seam. `gradient linear` (0→1 left to right) is
the one generator that does **not** tile — it has a hard edge at the wrap; use it under
a mask, or use `linear_tiling` (0→1→0) for a seamless ramp. `gradient radial` and
`wave rings` meet the edge mirror-symmetrically: continuous across the seam, and so
does `shape` (a shape larger than the tile is clipped at the edge, symmetrically).

### Units

Params are in **tile units**, never pixels, so the same recipe looks the same at 256²
and 2048² (iterate at a low `resolution`, ship at a high one):

- `blur.radius` is a **fraction of the tile width** — `0.01` is 1% of the texture;
  the pixel radius is `round(radius × resolution)`, capped at half the tile.
- `warp.strength` is a displacement in tiles: a channel at 1.0 pushes the read
  `strength / 2` of a tile.
- `bump_to_normal.strength` scales the slope measured in tile units (height change
  across one whole tile), not per pixel.
- `shape.size` and `shape.softness` are in tiles (`size = [0.5, 0.5]` is half the
  tile each way); under `tile`, that is a fraction of one cell.
- `white_noise` is the exception: its grain is one pixel by definition.

**Migration (#392, #394).** Recipes written before these changes bake differently:
`blur.radius` was whole pixels (`radius = 4` at 512² is now `radius = 0.008`);
`bump_to_normal.strength` was per pixel (divide an old value by the `resolution` it was
tuned at, e.g. `4.0` at 512² → `0.0078`); fractional counts in the table above, and odd
`checker.tiles`, now round; `mapping` now resamples bilinearly and, unless
`tiling = false`, rounds its scale and snaps its rotation.

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

Example — a riveted floor plate's normal map: a bevelled rounded panel with a
jittered grid of domed rivets added on top at half height, turned into normals:

```lua
Texture.Bake({
  resolution = 512, seed = 3,
  nodes = {
    { id = "panel", op = "shape", kind = "rounded_rect", size = {0.92, 0.92}, softness = 0.03 },
    { id = "dot", op = "shape", kind = "circle", size = {0.3, 0.3}, softness = 0.12 },
    { id = "rivets", op = "tile", count = 8, jitter = 0.1, inputs = {"dot"} },
    { id = "height", op = "mix", mode = "add", factor = 0.5, inputs = {"panel", "rivets"} },
    { id = "n", op = "bump_to_normal", strength = 0.4, inputs = {"height"} },
  },
  output = "n",
}, "out/plate_n.png", "normal")
```
