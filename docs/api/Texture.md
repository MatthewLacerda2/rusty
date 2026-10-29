## `Texture`

Agent-composed **procedural texture authoring** (#270): build a *recipe* — a small
DAG of curated ops — and **bake** it to a tiling `.png` a material map slot consumes.
This is the texture leg of the authoring layer and the shared spine (recipe →
op-runner → bake) the material/shader legs build on.

A recipe is authored as a Lua **table** whose shape mirrors the on-disk JSON document
one-to-one, so the same DAG describes a Lua-built recipe and one loaded from a file.
An `op` tag and an enum param (`kind`, `mode`, …) are read in any case (`"NOISE"`).
Output maps are **seamless by construction** (the procedural domain wraps, and every
periodic param is a whole count of repeats per tile — see *Tiling* below),
**resolution-independent** (`resolution` only chooses how finely the pattern is
sampled — see *Units* below) and **deterministic** — the same recipe + `seed` always
bakes a byte-identical PNG (every stochastic op draws from a seeded integer hash, never
wall-clock or unseeded RNG).

| Function | Signature | Returns |
|---|---|---|
| `Texture.Bake` | `(recipe, path, slot)` | the written `path` |
| `Texture.BakeSet` | `(recipe, prefix)` | `{ [slot] = path, … }` — one PNG per entry of the recipe's `outputs` |
| `Texture.ToJson` | `(recipe)` | the recipe's canonical JSON string |
| `Texture.Stats` | `(recipe, [resolution])` | `{ [node_id] = { min, max, mean, std, p1, p99 }, … }` for **every** node, each field an `{r, g, b, a}` array |
| `Texture.Preview` | `(recipe, path, [size = 128])` | `{ path, columns, order = { node ids } }` — a PNG contact sheet of every node |
| `Texture.Ops` | `()` | the op catalog: `{ {op, category, min_inputs, max_inputs, params = { {name, type, default, required, values} } }, … }` |

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
| `metallic_roughness` / `orm` | linear | **metallic → B, roughness → G**, R = 0 (glTF's packed metallic-roughness map) |
| `data` | linear, **16-bit** | — (height/masks/anything raw; the explicit way to get unencoded output) |

Every slot bakes 8 bits per channel except `data`, which bakes a **16-bit** PNG
(#1491): a height field used as displacement or a terrain heightmap keeps 65536
levels instead of terracing into 256 steps. The 16 bits are the file's: the renderer
loads it as a material map like any other PNG, but samples every map at 8 bits per
channel today, so the extra precision serves what reads the file itself (an
`image` node, a terrain or displacement bake), not what the GPU draws.

`orm` is only another name for the glTF metallic-roughness packing, not an
occlusion-roughness-metallic map: rusty's material has no occlusion map, so nothing
is written to R.

The returned `path` drops straight into a material slot, e.g.
`Material.SetTexture(id, Texture.Bake(recipe, "out/albedo.png", "base_color"))`.
Baking again to a path already on screen shows the new texels on the next frame —
the renderer re-reads every PNG a bake writes — and a material that named the path
before the file existed picks it up once it is baked (#689).

### A whole map set from one graph — `Texture.BakeSet`

A PBR material needs albedo, normal and roughness derived from the **same** pattern
(the mortar lines must match across maps). Give the recipe an `outputs` table — slot
→ node id — and `Texture.BakeSet` evaluates the graph **once** (shared upstream nodes
run once), bakes each output with its slot's encoding to `<prefix>_<slot>.png`, and
returns canonical slot → path:

```lua ignore
local maps = Texture.BakeSet({
  resolution = 1024, seed = 7,
  nodes = { ...one brick+noise graph... },
  outputs = { base_color = "albedo", normal = "n", metallic_roughness = "mr" },
}, "out/brick")
-- maps = { base_color = "out/brick_base_color.png", normal = "out/brick_normal.png",
--          metallic_roughness = "out/brick_metallic_roughness.png" }
```

Each map is **byte-identical** to baking that node alone with `Texture.Bake`. The
`outputs` keys are slot names as in the table below (aliases accepted; the result
and the file name use the canonical name: `albedo` → `base_color`, `orm` →
`metallic_roughness`). The result's slots feed `Material.DefineAsset` map keys:

| Result key | `Material.DefineAsset` key(s) |
|---|---|
| `base_color` | `base_color_map` |
| `emissive` | `emissive_map` |
| `normal` | `normal_map` |
| `roughness` | `roughness_map` |
| `metallic` | `metallic_map` |
| `metallic_roughness` | `metallic_map` **and** `roughness_map` (the same PNG) |
| `data` | — (no material slot) |

To skip the hand-off, put the whole recipe in the material as `maps`
(`Material.DefineAsset(name, { maps = recipe })`, see `Material.md`): it bakes
with `BakeSet` and fills these keys for you, and `Material.Rebake` re-bakes it.

Everything is checked before a file is written: a recipe with no `outputs`, an
unknown slot, two keys naming one slot (`albedo` and `base_color`), or an unknown
node id is an error and nothing is baked. `outputs` is optional and ignored by
`Texture.Bake`, which still bakes the single `output`.

### The recipe shape

```lua
{
  resolution = 1024,        -- power-of-two canvas; sane default 1024
  -- size = {2048, 512},    -- a non-square canvas in place of resolution (see *Non-square bakes*)
  seed = 42,                -- folded into every stochastic op (default 0)
  output = "n1",            -- output node id (optional; defaults to the last node)
  -- outputs = { base_color = "n1", … },  -- slot → node id, for Texture.BakeSet
  nodes = {
    { id = "n0", op = "noise", kind = "fbm", scale = 8.0, octaves = 4 },
    { id = "n1", op = "color_ramp", inputs = {"n0"},
      stops = { { pos = 0.0, color = {0.1,0.05,0.0,1} },
                { pos = 1.0, color = {0.6,0.4,0.2,1} } } },
  },
}
```

Each node has a stable `id`, an `op` tag, the op's params as sibling fields, and an
`inputs` array of upstream node ids (in order; omit it on a generator). A node's
`inputs` must hold as many ids as its op accepts (`Texture.Ops()`' `min_inputs` to
`max_inputs`): a short or surplus list is refused before anything bakes, naming the
node — `node "mr" (op "mix") takes 2 to 3 inputs, but 1 given` — so a dangling wire
never bakes a silently black or flat map. **Unknown keys are errors**
everywhere in a recipe — on the recipe itself, on a node, on a ramp stop: `{ op =
"noise", scael = 8 }` is refused with a message naming `scael` and the params `noise`
takes, rather than baking with the default scale. The runner evaluates the DAG
in dependency order in linear `[f32]` RGBA, then the bake encodes/packs per `slot`.

### Checking a recipe — `Texture.Stats` and `Texture.Preview`

You never see a recipe's intermediate maps, so **read them before you tune them**.
Both verbs evaluate the whole recipe once, every node (not only the ones the output
reads), and take each node's numbers or thumbnail as it finishes, so they cost one
bake and hold no more images than a bake does.

`Texture.Stats(recipe, [resolution])` returns, per node id, the per-channel `min`,
`max`, `mean`, `std` (population) and the exact 1st/99th percentiles `p1`/`p99`
(nearest rank: all but the outer 2% of pixels lie between them). Each is an
`{r, g, b, a}` array of linear values, as the runner works in them, before any
slot's encoding. `resolution` overrides the recipe's for speed: params are in tile
units (*Units*, below), so the numbers barely move between 128² and 2048². On a
`size` recipe it sets the longer side and keeps the aspect, as `Material.Rebake`
does (*Non-square bakes*, below).

**Read `Stats` before setting `color_ramp` stops or `map_range` bounds.** A noise
does not span 0–1: fBM is normalised by its total amplitude, so it clusters around
0.5, and the more octaves the tighter (at `lacunarity` 2, `gain` 0.5):

| `octaves` | `p1` – `p99` | `std` |
|---|---|---|
| 1 | 0.27 – 0.72 | 0.106 |
| 4 | 0.35 – 0.64 | 0.065 |
| 8 | 0.36 – 0.64 | 0.062 |

A ramp with stops at 0 and 1 over a 4-octave fBM lands nearly every pixel near its
middle colour: a flat, washed-out map. Put the stops at the input's `p1` and `p99`,
or stretch it first with `map_range { from_min = p1, from_max = p99, to_min = 0,
to_max = 1 }`.

```lua
local s = Texture.Stats({ resolution = 1024, nodes = {
  { id = "n", op = "noise", kind = "fbm", scale = 8, octaves = 4 } } }, 256)
print(s.n.p1[1], s.n.p99[1])  -- red channel: about 0.35, 0.64
```

`Texture.Preview(recipe, path, [size = 128])` writes one PNG at `path`: every node's
image shrunk to a tile whose longer side is `size`, in the recipe's aspect (a box
filter in linear), in the recipe's node order,
left to right then top to bottom, in a near-square grid of `columns` columns (unused
cells of the last row are black). No text is drawn: tile `k` (from 1) is
`order[k]`, so open the PNG and match tiles to ids. Every tile is **sRGB-encoded**,
data included, the way Blender's node previews show everything through the view
transform: a 0.5 mask shows as 188, not 128. Alpha is not shown.
`size` defaults to 128, or the recipe's longer side when that is smaller; a `size`
above that side is an error, since a tile larger than the image adds nothing.
Both verbs refuse a recipe whose `output` or `outputs` names a missing node, as the
bakes do.

```lua no-run
local p = Texture.Preview(recipe, "out/preview.png", 96)
-- p = { path = "out/preview.png", columns = 3, order = { "h", "paint", … } }
```

### The op-set (curated "Blender-lite")

Grouped like Blender's node menus; `op` is the tag string in each node.

**Ask the engine, not this page.** `Texture.Ops()` returns the live catalog — the
list below is a summary of it. Each entry has the `op` tag, its `category`
(`generator` | `color` | `vector` | `math` | `filter`), how many inputs it accepts
(`min_inputs` to `max_inputs`; a node outside that range is an error), and its
`params`. Each param has a
`name`, a `type` (`number` | `integer` | `bool` | `color` | `vec2` |
`enum` | `stops` | `path`, a project-relative file path), `required`, a `default` when it has one (absent when required),
`socket` (whether it takes a link to another node, *Linked params* below), and for an
`enum` the accepted `values`. Units and rounding are the ones *Tiling*
and *Units* below describe. A unit test holds the catalog to the recipe parser, so
it can't name an op or param the parser doesn't take, or a default it doesn't apply.

**Colours.** A `color` param (and a ramp stop's `color`) takes either a **linear**
RGBA array, `{0.5, 0.5, 0.5, 1}`, or an **sRGB** hex string, `"#808080"` /
`"#808080ff"`, decoded to linear when the recipe is parsed (alpha stays linear), like
Blender's hex field. They are not the same colour: the array `{0.5, 0.5, 0.5, 1}` bakes
to 188/255, a light grey, while `"#808080"` is linear ≈ 0.216 and bakes back to 128 —
so a colour picked from a reference goes in as hex. `Texture.ToJson` writes every colour
as its linear array. A malformed hex is an error naming the param.

**Seeds.** Every stochastic op (`noise`, `voronoi`, `brick`, `white_noise`, `tile`)
takes an optional integer `seed` (default 0), folded into the recipe `seed`. Two nodes
that differ only in `seed` give independent patterns — the warp's X and Y noises, dirt
and chips at the same scale — and 0 keeps the recipe seed, so a recipe without node
seeds bakes as it always did.

```lua
for _, o in ipairs(Texture.Ops()) do
  if o.op == "brick" then
    for _, p in ipairs(o.params) do print(p.name, p.type, p.default) end
  end
end  -- rows number nil / cols number nil / mortar number 0.05
```

- **Generators** (no inputs): `constant {color}`; `noise
  {kind="perlin"|"fbm"|"ridged"|"turbulence", scale, octaves=1, lacunarity=2, gain=0.5,
  seed=0}`
  (`ridged` is `1 − |n|` squared — rock veins, scratches; `turbulence` is `|n|` —
  smoke, grime; the fBM family stacks `octaves`, each `lacunarity`× the frequency and
  `gain`× the amplitude of the last; `perlin` is one octave); `voronoi {scale,
  output="distance"|"cells"|"f2"|"edges", metric="euclidean"|"manhattan"|"chebyshev",
  randomness=1, seed=0}` (`f2` is the distance to the
  second-nearest point, `edges` is F2 − F1 — ~0 on cell borders: cracks, dried mud,
  cobble outlines; `metric` is how distance is measured: `euclidean` gives round cells,
  `manhattan` (`|dx| + |dy|`) diamond-cut ones, `chebyshev` (`max(|dx|, |dy|)`) blocky,
  straight-edged ones — stones, tiles, flagstones; `randomness` 0 is a regular grid, 1
  full jitter); `gradient
  {kind="linear"|"linear_tiling"|"radial"}`; `wave {kind="bands"|"rings", frequency}`;
  `brick {rows, cols, mortar=0.05, output="mask"|"random"|"bevel", seed=0}` (`mortar` is the
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
  `white_noise {seed=0}`; `image {path, encoding="srgb"|"linear"}` (an image file,
  see *Image files* below).
- **Color** (1 input; `mix` takes 2 or 3): `color_ramp {stops,
  interpolation="linear"|"constant"|"ease"}` (as Blender's ColorRamp: `constant` holds
  each stop's colour from its position up to the next stop — hard bands for toon,
  stylized and posterized looks — and `ease` is a smoothstep between stops, soft into
  and out of each colour); `mix {mode,
  factor=1}` with `mode` ∈
  `mix/add/multiply/screen/overlay/subtract/difference/lighten/darken/divide` —
  inputs `{a, b, mask}`: the result is `a` blended toward `mode(a, b)` by `factor`,
  and an optional third input `mask` makes that per pixel (`factor × mask.red`; black
  keeps `a`, white takes the full blend). `lighten` is the per-channel max and `darken`
  the min — the way to combine height fields (rivets over a panel, stones over mortar),
  where `add` overshoots; `divide` is `a / b`, and a channel of `b` at (near) 0 leaves `a`;
  `invert`; `bright_contrast {bright, contrast}` (contrast pivots at linear 0.5, like
  Blender's node: right for data maps, but that is a light grey on screen, so for a
  colour map use hex ramp stops or `auto_levels` → `color_ramp`); `hue_sat_value {hue,
  sat, value}`; `gamma {gamma}`; `auto_levels` (stretches RGB so the input's minimum
  maps to 0 and its maximum to 1 — one range across R, G and B, so a colour keeps its
  hue, like Substance's Auto Levels; alpha untouched, a flat input unchanged). Noise
  uses only part of the range (a 4-octave fBM spans about 0.35–0.64), so a ramp fed
  straight from it shows only its middle; `auto_levels` before the `color_ramp` uses
  the whole ramp, whatever the input's spread.
- **Vector / normal**: `mapping {scale=[x,y], rotation, translation=[x,y],
  tiling=true}` (bilinear, wrapped resample; `rotation` in turns); `bump_to_normal
  {strength}` (height → tangent-space normal); `combine_rgb` (1 to 3 grayscale
  inputs → R, G, B; a channel without an input reads 0); `separate_rgb
  {channel=0..3}`; `warp {strength}` (2 inputs: samples input 1 displaced by input 2's R/G, centred on 0.5 so mid-grey moves nothing; `strength` is in
  tile units; feed input 2 a `combine_rgb` of two different noises — a grayscale one has
  R = G and only pushes along the diagonal); `normal_blend {strength=1}` (2 inputs:
  base, detail — two normal maps, as `bump_to_normal` writes them, combined by
  Reoriented Normal Mapping: the base is turned by the detail's tilt, so scratches over a
  panel follow the panel's bevel. `strength` scales the detail's tilt, 0 leaving the
  base; a flat normal on either side returns the other. Never `mix` normal maps: adding
  or averaging their RGB flattens them); `tile {count, jitter=0, rotation_jitter=0,
  scale_jitter=0, seed=0}` (1 input: stamps the input once per cell of a `count`×`count` grid;
  each cell is seeded from the recipe and node `seed` with an offset of up to ±`jitter`/2 cells,
  a turn of up to ±`rotation_jitter`/2 turns and a scale in `1 ± scale_jitter/2`, each
  jitter clamped to 0–1. The input is read as **one stamp** — the unit square, nothing
  outside it — so give it empty margins (a `shape` smaller than the tile); a stamp
  pushed over its cell's border is drawn into the neighbour cell, and overlapping
  stamps keep the brighter value per channel. Rivets are `shape circle` → `tile`).
- **Converter / math**: `math {func, value=0.5}` (1 or 2 inputs) with `func` ∈
  `add/subtract/multiply/divide/power/min/max/abs/fract/sqrt` — the right-hand side
  is `value`, or with a second input wired, that input's matching channel per pixel
  (`value` is then ignored, Blender's Math with both sockets linked). `abs`, `fract`
  and `sqrt` have no right-hand side, so they take exactly 1 input; `map_range {from_min,
  from_max, to_min, to_max}`; `clamp {min, max}`; `rgb_to_bw`.
- **Filter**: `blur {radius, kind="gaussian"|"box"}` (separable; wraps, so it stays
  tiling; `gaussian`, the default, is a round falloff with `σ = radius / 3`: three
  box passes per axis matched to that variance, each `O(n)` whatever the radius, so
  it costs about three boxes and its kernel stops a little short of `radius` (about
  10 px for 12); under about 2.5 px it is one exact 3-tap pass, so even a 1-pixel blur
  softens; `box` is one flat window, a third of the cost, and leaves square artefacts
  on soft masks and normals);
  `cavity {radius}` (height → `1 − max(blur(h) − h, 0)` with a Gaussian blur: white on flat and raised
  ground, dark in crevices — multiply into albedo for baked occlusion, `invert` it
  for a dirt mask); `curvature {radius, strength=1}` (height → `0.5 + strength × (h −
  mean of h on a ring radius away)`: above mid-grey on convex edges — edge wear —
  below in concave grooves — grime; `map_range` it into a hard mask). Both read the
  input's red as height and wrap, so the masks tile. `distance {radius}` (mask → bevel:
  the input's red above 0.5 is inside, and each inside pixel reads its distance to the
  mask's edge divided by `radius`, clamped to 1; outside reads 0. It rounds **any**
  mask — a Voronoi cell, a thresholded noise, a stamp — the way `shape`'s `softness`
  and brick's `bevel` round their own; feed it to `bump_to_normal`. Straight, it is a
  chamfer; through `math` `func = "sqrt"` (a curve steepest at the rim and flat on
  top) it is a rounded, convex edge. Avoid an `ease` `color_ramp` here: it is flat at
  the rim too, so the edge reads as a pillow. It wraps, so a tiling mask gives a
  tiling bevel).

### Linked params — a node drives a value

Blender lets any input socket take another node. A curated set of rusty's params
does the same: in place of a number, the param takes a **link**, and its value then
varies across the map.

```lua
strength = { node = "damage", min = 0, max = 0.2 }  -- min + (max − min) × damage.red
```

`min` and `max` default to 0 and 1. A link reads the linked node's **red** channel,
like every mask input; to read another channel, link a `separate_rgb` of it. The
linked node is an edge of the graph like an input: it is evaluated first, a cycle
through it is refused, and `Texture.Stats` and `Texture.Preview` show it like any
node. A link to a node that doesn't exist, a cycle through a link and a link on a
param that takes none are each refused before anything is written, the last naming
the params of that op that do link. A link to a `constant` bakes exactly like its
number.

**The params that link** (`socket = true` in `Texture.Ops()`, the list to trust):

- `warp.strength`, `bump_to_normal.strength`, `curvature.strength`,
  `normal_blend.strength`: read at each pixel;
- `blur.radius`, `distance.radius`: read at each pixel. A blur's radius varies
  smoothly: sharp where the link is dark, soft where it is bright, each pixel
  averaging its own window in constant time whatever its width. A Gaussian is still
  three passes;
- `tile.jitter`, `tile.rotation_jitter`, `tile.scale_jitter`, `voronoi.randomness`:
  read **once per cell**, at the cell's centre, wrapped, so each cell keeps one
  stamp or one point and the result still tiles;
- `shape.softness`, `noise.gain`: read at each pixel;
- `hue_sat_value.hue`, `.sat`, `.value`, `bright_contrast.bright`, `.contrast`,
  `gamma.gamma`: read at each pixel.

**What never links:** a whole count (`scale`, `octaves`, `lacunarity`, `rows`,
`cols`, `tiles`, `count`, `frequency`). A count that changed from pixel to pixel
could not stay whole, so it would open seams or cut visibly where it stepped. Nor do
`mapping`'s params (a per-pixel translation is `warp`, and a per-pixel scale or
rotation would break the tiling), enums, colours or `size`. `mix.factor` and
`math.value` already take an image through an input.

Disorder that grows over a damaged area: tiles laid in an ordered grid on clean
ground and scattered where a damage mask is white. Mixing two `tile` ops by the mask
would double the stamps along the transition; one linked `tile` keeps one stamp per
cell.

```lua
{ id = "tile",   op = "shape", kind = "rounded_rect", size = {0.9, 0.9}, softness = 0.1 },
{ id = "damage", op = "noise", kind = "fbm", scale = 3, octaves = 4, seed = 5 },
{ id = "edge",   op = "map_range", from_min = 0.55, from_max = 0.65, to_min = 0, to_max = 1,
  inputs = {"damage"} },
{ id = "mask",   op = "clamp", min = 0, max = 1, inputs = {"edge"} },
{ id = "tiles",  op = "tile", count = 8, inputs = {"tile"},
  jitter          = { node = "mask", max = 0.6 },
  rotation_jitter = { node = "mask", max = 0.15 },
  scale_jitter    = { node = "mask", max = 0.3 } },
```

### Image files — `image`

`{ op = "image", path = "textures/crate.png", encoding = "srgb" }` starts a graph from
an existing map, like Blender's *Image Texture* node: a texture extracted from an
imported model, a provider-generated image, an earlier bake, a hand-made mask or logo
— to recolour, weather, turn into a normal map, or use as a mask.

- `path` is taken from the **project root**, as `require` takes its scripts. A file
  outside the project is read, but warns once per file in the console: it is an
  input the seed doesn't cover, and may be missing on another machine.
- `encoding = "srgb"` (the default, a colour image) decodes RGB from sRGB to linear;
  `"linear"` (a mask, a height, a normal map) reads every channel as stored. Alpha is
  always as stored. 16-bit files keep their precision. png, jpeg and tga are read;
  the format comes from the file's extension.
- The file is resampled **bilinearly, wrapped**, to the recipe's `resolution` (or
  `size`, each axis on its own), so the recipe stays resolution-independent. A file
  already at that size reads back texel for texel.
- Its **tiling is the file's own**: a photo that doesn't tile keeps its seam (the
  wrap joins its right edge to its left). Every other op still wraps, so a seamless
  source stays seamless through the graph.
- Any format the importer reads: png, jpeg, tga.
- A file that can't be read or decoded is an **error** naming the node and the path,
  never a black canvas. Every verb reads the files when it runs (`Texture.Bake`,
  `BakeSet`, `Stats`, `Preview`, and a material's maps via `Material.DefineAsset` /
  `Material.Rebake`), so a rebake picks up an edited source.

```lua no-run
Texture.Bake({ resolution = 512, nodes = {
  { id = "logo", op = "image", path = "textures/logo.png", encoding = "linear" },
  { id = "h", op = "blur", radius = 0.01, inputs = { "logo" } },
  { id = "n", op = "bump_to_normal", strength = 0.3, inputs = { "h" } } },
}, "textures/logo_n.png", "normal")
```

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
`image` tiles exactly as well as its file does.

### Units

Params are in **tile units**, never pixels, so the same recipe looks the same at 256²
and 2048² (iterate at a low `resolution`, ship at a high one):

- `blur.radius` is a **fraction of the tile** — `0.01` is 1% of the texture;
  the pixel radius is `radius × resolution` (a `box` rounds it), capped at half the
  tile, and on a `size` recipe `radius × width` across and `radius × height` down. A `gaussian` reaches about `radius`, a `box` exactly its rounded radius.
- `warp.strength` is a displacement in tiles: a channel at 1.0 pushes the read
  `strength / 2` of a tile.
- `bump_to_normal.strength` scales the slope measured in tile units (height change
  across one whole tile), not per pixel. On a `size` recipe the unit is a tile of the
  **longer** side on both axes, so the same height step between neighbouring pixels
  slopes the same across and down, as on a mesh mapped at an even texel density.
- `shape.size` and `shape.softness` are in tiles (`size = [0.5, 0.5]` is half the
  tile each way); under `tile`, that is a fraction of one cell.
- `distance.radius` is in tiles: the distance from the edge that reads 1.
- `cavity.radius` is a tile fraction like `blur.radius`; `curvature.radius` is a
  distance in tiles, sampled bilinearly (so sub-pixel radii stay smooth).
- `white_noise` is the exception: its grain is one pixel by definition (on each axis).

### Non-square bakes — `size`

`size = {w, h}` bakes a `w × h` map in place of `resolution`'s square: a trim sheet
(a 2048×512 strip of mouldings and panels), a wide decal, signage, a UI strip. Each
side must be a positive power of two; any other value is refused when the recipe is
read (`size side 300 is not a positive power of two`). With `size` set,
`resolution` may be left out, and is ignored if given.

The units are **relative, as in Substance Designer**: the domain stays the unit
tile, and a non-square map samples it with more pixels along one axis, so the
pattern **stretches**. Every param keeps its meaning (a `blur.radius` of 0.01 is 1%
of each side, a `distance.radius` is in tiles of each axis), and every periodic
count stays a whole number per tile, so the map still tiles on both axes. To get
square features back, scale the counts by the aspect: on a 4:1 strip, `cols = 4 ×
rows` gives square bricks, and `scale = {4, 1}` on a `mapping` squares up its input.

```lua no-run
Texture.Bake({ size = {2048, 512}, seed = 1, nodes = {
  -- 4:1, so 8 columns over 2 rows lays bricks as wide as they are tall.
  { id = "b", op = "brick", rows = 2, cols = 8, mortar = 0.04 },
  { id = "h", op = "blur", radius = 0.004, inputs = { "b" } },
  { id = "n", op = "bump_to_normal", strength = 0.4, inputs = { "h" } } },
}, "textures/trim_n.png", "normal")
```

`Material.Rebake(name, resolution)` and `Texture.Stats(recipe, resolution)` keep a
`size` recipe's aspect: both sides scale by the same power of two so the **longer**
side equals `resolution` (`{2048, 512}` at 256 becomes `{256, 64}`). A
`resolution` that isn't a power of two, or one that would shrink the shorter side
below a pixel, is an error. `Texture.Preview` tiles keep the aspect too.

**Migration (#392, #394).** Recipes written before these changes bake differently:
`blur.radius` was whole pixels (`radius = 4` at 512² is now `radius = 0.008`);
`bump_to_normal.strength` was per pixel (divide an old value by the `resolution` it was
tuned at, e.g. `4.0` at 512² → `0.0078`); fractional counts in the table above, and odd
`checker.tiles`, now round; `mapping` now resamples bilinearly and, unless
`tiling = false`, rounds its scale and snaps its rotation.

Example — a packed metallic-roughness map baked in one shot (red = metallic,
green = roughness, routed to B/G by the `metallic_roughness` slot):

```lua no-run
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

```lua no-run
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

Example — worn, grimy painted metal: curvature finds the raised edges, a noisy mask
chips the paint off them to show bare metal, and cavity darkens the grooves:

```lua no-run
Texture.Bake({
  resolution = 512, seed = 4,
  nodes = {
    { id = "h", op = "brick", rows = 4, cols = 2, mortar = 0.06 },        -- panel height
    { id = "paint", op = "constant", color = {0.20, 0.32, 0.18, 1} },
    { id = "metal", op = "constant", color = {0.55, 0.56, 0.58, 1} },
    { id = "curv", op = "curvature", radius = 0.01, strength = 3, inputs = {"h"} },
    { id = "edge", op = "map_range", from_min = 0.6, from_max = 0.7,
      to_min = 0, to_max = 1, inputs = {"curv"} },
    { id = "chips", op = "noise", kind = "fbm", scale = 8, octaves = 4 },
    { id = "wear", op = "mix", mode = "multiply", inputs = {"edge", "chips"} },
    { id = "worn", op = "mix", mode = "mix", inputs = {"paint", "metal", "wear"} },
    { id = "cav", op = "cavity", radius = 0.02, inputs = {"h"} },
    { id = "out", op = "mix", mode = "multiply", inputs = {"worn", "cav"} },
  },
  output = "out",
}, "out/panel_albedo.png", "albedo")
```

Example — cobblestones: Chebyshev Voronoi cells (blocky, straight-edged) thresholded
into a stone mask, each stone rounded by `distance`, and a fine grain laid over the
stones' normals with `normal_blend`:

```lua no-run
Texture.Bake({
  resolution = 512, seed = 2,
  nodes = {
    { id = "cells", op = "voronoi", scale = 6, output = "edges", metric = "chebyshev" },
    { id = "stones", op = "map_range", from_min = 0.06, from_max = 0.1,
      to_min = 0, to_max = 1, inputs = {"cells"} },                 -- 1 on stones, 0 in the gaps
    { id = "bevel", op = "distance", radius = 0.04, inputs = {"stones"} },
    { id = "round", op = "math", func = "sqrt", inputs = {"bevel"} }, -- a rounded edge
    { id = "n", op = "bump_to_normal", strength = 0.5, inputs = {"round"} },
    { id = "grain", op = "noise", kind = "fbm", scale = 24, octaves = 3 },
    { id = "gn", op = "bump_to_normal", strength = 0.1, inputs = {"grain"} },
    { id = "out", op = "normal_blend", strength = 0.5, inputs = {"n", "gn"} },
  },
  output = "out",
}, "out/cobblestone_n.png", "normal")
```
