## `Texture`

**Snyder**, rusty's procedural texture graph (Substance Designer / Blender's shader
nodes, baked to PNG). Snyder is the tool's name; its API is this `Texture` namespace.

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
| `Texture.BakeMeshMaps` | `(model, maps, prefix, [resolution = 1024])` | `{ [map] = path, … }` — one PNG per mesh map, baked from a model's UV0 |
| `Texture.ToJson` | `(recipe)` | the recipe's canonical JSON string |
| `Texture.Stats` | `(recipe, [resolution])` | `{ [node_id] = { min, max, mean, std, p1, p99 }, … }` for **every** node, each field an `{r, g, b, a}` array |
| `Texture.Preview` | `(recipe, path, [size = 128])` | `{ path, columns, order = { node ids } }` — a PNG contact sheet of every node |
| `Texture.Ops` | `()` | the op catalog: `{ {op, category, min_inputs, max_inputs, params = { {name, type, default, required, values} } }, … }`, then the project's graphs (`op = "graph"`, with `path` and `inputs`; *Graphs* below) |

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
| `metallic_roughness` / `orm` | linear | reads **metallic from the input's R, roughness from its G** (input B is ignored), and writes them as glTF's packed map: **metallic → B, roughness → G**, R = 0 |
| `data` | linear, **16-bit** | — (height/masks/anything raw; the explicit way to get unencoded output) |

Every slot bakes 8 bits per channel except `data`, which bakes a **16-bit** PNG
(#1491): a height field used as displacement or a terrain heightmap keeps 65536
levels instead of terracing into 256 steps. The 16 bits are the file's: the renderer
loads it as a material map like any other PNG, but uploads every map at 8 bits per
channel at most today (and block-compressed below that once the import cache has
made its copy, unless its `compression` is `"None"`; see `Assets.md`), so the extra precision serves what reads the file itself (an
`image` node, a terrain or displacement bake), not what the GPU draws.

`orm` is only another name for the glTF metallic-roughness packing, not an
occlusion-roughness-metallic map: rusty's material has no occlusion map, so nothing
is written to R.

**The target folder.** `Bake` and `BakeSet` make the missing folders above `path`
(or above `prefix`) when they lie inside the project, so a new set can bake to
`assets/textures/<new set>/brick` at once (#1850). A target outside the project is
written only into a folder that already exists. A write that still fails names the
verb and the path: `Texture.BakeSet: cannot write <path>: <os error>`.

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

### Maps baked from a model — `Texture.BakeMeshMaps`

The ops' `curvature`, `cavity` and `ambient_occlusion` read a height field in texture
space, so they know nothing of a model's real edges. `Texture.BakeMeshMaps` is
Substance's *Bake Mesh Maps*: it bakes data about one model's surface into its
**UV0**, and a recipe reads each map with the `image` op (`encoding = "linear"`), so
edge wear follows the model's chamfers and grime its real recesses (#1849).

```lua no-run
local maps = Texture.BakeMeshMaps("assets/models/kolvik/generator.blend::Generator",
  { "curvature", "ao", "world_normal" }, "assets/textures/kolvik/generator_", 1024)
-- maps.curvature == "assets/textures/kolvik/generator_curvature.png"
Texture.Bake({ resolution = 1024, nodes = {
  { id = "edges", op = "image", path = maps.curvature, encoding = "linear" },
  { id = "wear", op = "levels", in_low = 0.55, in_high = 0.8, inputs = { "edges" } },
} }, "assets/textures/kolvik/generator_wear.png", "data")
```

- `model` is an asset reference as `Scene.Instantiate` takes it: `path::SubObject`,
  or a bare model path when the file holds one mesh (one holding several is an
  error that lists them). The mesh is baked **in its own space**, as imported:
  "world" means the model's frame before any scene transform.
- `maps` lists any of the names below (case-insensitive; a map named twice bakes
  once). Each is written to `<prefix><map>.png` as soon as it is baked, the folders
  above it made first as `Bake` makes them (a `prefix` ending in `/` names a folder
  of its own). `resolution` is the square map's edge, 1 to 8192. Memory grows with
  its square: at most about 1.2 GiB at 4096 and 4.6 GiB at 8192 (every texel
  covered), whatever the number of maps, since one map is held at a time.
- Every map but `id` is a 16-bit `data` PNG; `id` is 8-bit.

| Map | What a texel holds | Off the islands |
|---|---|---|
| `curvature` | 0.5 flat; brighter towards a convex edge (1 on a right-angled one), darker towards a concave one (0) | 0.5 |
| `ao` | the share of 64 cosine-weighted rays that meet no part of the model within its bounds' diagonal: 1 open, 0 enclosed | 1 |
| `world_normal` | the surface normal, `n * 0.5 + 0.5` per axis (Y up) | 0.5 |
| `position` | the surface point, normalised per axis to the model's bounds (a flat axis reads 0.5) | 0 |
| `thickness` | how far 64 rays cast into the model (around the inverted normal) travel before they leave it, averaged, over the bounds' diagonal; a ray that never leaves counts the whole diagonal: 1 thick, towards 0 thin | 1 |
| `id` | one fully saturated colour per material, hues a golden-ratio step apart (a triangle with no material red, then the model's materials in order); two runs drawn with one material share its colour | black |

The bake's settings are Substance's defaults and are **fixed today**: no argument
or recipe changes them. They are 64 rays per texel, an occluder distance of the
whole bounds diagonal, and a **16-texel margin** around every UV island (a
continuous map's margin averages its border, the ID map's copies it, so no margin
holds a colour between two materials). Rays are drawn around the interpolated
normal, and one that would leave under the triangle's own face is dropped, so on a
smooth low-poly mesh no ray starts under the face it leaves; a face wound
against its normals (an `.obj` face listed clockwise from its `vn`) draws them
around its face normal instead. A `curvature` crease lights a band 2%
of the bounds diagonal wide, never narrower than 3 texels: each edge two faces
share is a crease of strength `angle / 90°` (capped at 1), corners welded by
position first so a UV or normal seam never breaks one. A texel sums the convex
creases within the band (each fading to nothing at its edge), and apart the
concave ones, each sum held to 1: so a right angle reads alike as one sharp edge,
a chamfer or a rounded bevel of many small creases, while the bevel is narrow
beside the band (a bevel wider than the band reads as the gentle curve it is).
A boundary edge is no
crease, nor is the outline of a double-sided face (two faces with opposite
normals), and a texel reads only the creases of **its own connected surface**: a
floor beside a separate crate, or a hull under a floating panel, gets no band
from the other's edges. The band is a straight-line distance in 3D, not along
the surface: inside one connected mesh a crease still reaches faces across a
gap narrower than the band (between two fins, inside a slot, across a corridor
of a level-sized mesh), so model such parts as separate surfaces or keep the gap
wider than the band.

Only the part of UV0 inside the 0..1 square is baked. Where UVs overlap, a texel
takes the first triangle in index order. A model without UV0 (its texture UVs
enclose no area), without triangles, or with a non-finite position is an error
naming the verb and the model, never a black map. The bake is deterministic: the
same model and arguments write the same bytes, however many threads ran it.

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
node — `node "mr" (mix): takes 2 to 3 inputs, but 1 given` — so a dangling wire
never bakes a silently black or flat map. Every error about one node starts the
same way, with its id and op: `node "n" (noise): unknown key "scael"; …`,
`node "inv" (invert): references unknown input "ghost"`. **Unknown keys are errors**
everywhere in a recipe — on the recipe itself, on a node, on a ramp stop: `{ op =
"noise", scael = 8 }` is refused with a message naming `scael` and the params `noise`
takes, rather than baking with the default scale. The runner evaluates the DAG
in dependency order in linear `[f32]` RGBA, then the bake encodes/packs per `slot`.
A node `id` holds no `/` or `.`: those name a graph's inner nodes and outputs. A
recipe may also declare `params` and `inputs`, which make it a graph another recipe
instances as one node (*Graphs* below).

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

The subset of Blender's texture nodes a PBR material needs; `op` is the tag string in
each node, and `category` sorts the ops into Blender's node menus.

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

**Seeds.** Every stochastic op (each one listed below with `seed=0`) takes an
optional integer `seed` (default 0), folded into the recipe `seed`. Two nodes
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

One bullet per op, in alphabetical order; each names its `category` and the inputs it
takes (none for a generator).

A few notes hold for a whole family. The colour ops are pixel to pixel. Noise uses
only part of the range (a 4-octave fBM spans about 0.35–0.64), so a ramp fed straight
from it shows only its middle; `auto_levels` before the `color_ramp` uses the whole
ramp, whatever the input's spread. The filters wrap, so a tiling input gives a tiling
result (but for `flood_gradient` on a band that loops the tile, below); `ambient_occlusion`, `cavity` and `curvature` read the input's red as height (for a model's real edges and cavities, bake its mesh maps: `Texture.BakeMeshMaps`),
and `flood_fill` reads it as a mask, its readers the islands it found.

<!-- sorted: one bullet per op, so two branches adding ops land on different lines -->
- `ambient_occlusion {radius, depth=0.1, quality=8}` (*filter*, 1 input): height →
  occlusion by horizon search, like Substance's *Ambient Occlusion (HBAO)*: from each
  pixel it walks `quality` directions (1–64), 16 samples each out to `radius` tiles,
  and darkens by the sky the steepest rise hides; `depth` is how tall white stands, in
  tiles. Unlike `cavity`, it darkens the foot of a tall shape and the floor of a wide
  groove. Multiply it into albedo or bake it as the occlusion map. Its cost is
  `pixels × quality × 16` samples, whatever the radius: at 2048² and quality 8, about
  15 s of one core, split across cores by rows; iterate at a low `resolution`.
- `anisotropic_noise {scale, anisotropy=8, angle=0, seed=0}` (*generator*): value
  noise stretched into fibres, like Substance's *Anisotropic Noise* (Blender's *Gabor
  Texture* look): about `scale` fibres across the tile, each `anisotropy` (at least
  1) times longer than wide, running at `angle` turns (0 along `u`, 0.25 down `v`).
  Brushed metal, wood grain, fabric, scratches, at any angle, without stretching a
  noise through `mapping`. To tile, the angle **snaps** to the nearest direction of
  whole steps `(a, b)` (both at most 8) no longer than `scale / anisotropy` (a longer
  step would cut the fibres shorter than asked; the axes and diagonals always fit):
  within 3.6° of `angle` once `scale / anisotropy` is 8.1 or more, 5.7° at 6, 9.2° at
  3.2, and 22.5° (axes and diagonals only) under 2.3. Raise `scale` for a truer
  angle. On an axis (angle 0 or 0.25 snapped) a fibre is at most half a tile long,
  whatever `anisotropy` asks: one lattice cell along the whole tile would repeat
  itself into ruler-straight stripes. Cost: about 0.2 s of one core at 2048², under a one-octave `noise`.
- `auto_levels` (*color*, 1 input): stretches RGB so the input's minimum maps to 0 and
  its maximum to 1 — one range across R, G and B, so a colour keeps its hue, like
  Substance's Auto Levels; alpha untouched, a flat input unchanged.
- `blur {radius, kind="gaussian"|"box"}` (*filter*, 1 input): separable; wraps, so it
  stays tiling; `gaussian`, the default, is a round falloff with `σ = radius / 3`:
  three box passes per axis matched to that variance, each `O(n)` whatever the
  radius, so it costs about three boxes and its kernel stops a little short of
  `radius` (about 10 px for 12); under about 2.5 px it is one exact 3-tap pass, so
  even a 1-pixel blur softens; `box` is one flat window, a third of the cost, and
  leaves square artefacts on soft masks and normals.
- `brick {rows, cols, mortar=0.05, output="mask"|"random"|"bevel", seed=0}`
  (*generator*): `mortar` is the gap as a fraction of one brick, reading 0 in every
  output; `mask` is 1 on brick faces, `random` a seeded value per brick — feed a
  `color_ramp` for per-brick colour or roughness variation — and `bevel` a ramp from
  0 at the mortar to 1 on the brick's centre line, a rounded-brick height for
  `bump_to_normal`.
- `bright_contrast {bright, contrast}` (*color*, 1 input): contrast pivots at linear
  0.5, like Blender's node: right for data maps, but that is a light grey on screen,
  so for a colour map use hex ramp stops or `auto_levels` → `color_ramp`.
- `bump_to_normal {strength}` (*vector*, 1 input): height → tangent-space normal.
- `cavity {radius}` (*filter*, 1 input): height → `1 − max(blur(h) − h, 0)` with a
  Gaussian blur: white on flat and raised ground, dark in crevices — multiply into
  albedo for baked occlusion, `invert` it for a dirt mask.
- `checker {tiles, color_a, color_b}` (*generator*).
- `clamp {min, max}` (*math*, 1 input).
- `color_ramp {stops, interpolation="linear"|"constant"|"ease"}` (*color*, 1 input):
  as Blender's ColorRamp: `constant` holds each stop's colour from its position up to
  the next stop — hard bands for toon, stylized and posterized looks — and `ease` is
  a smoothstep between stops, soft into and out of each colour.
- `combine_rgb` (*vector*, 1 to 3 inputs): grayscale inputs → R, G, B; a channel
  without an input reads 0.
- `constant {color}` (*generator*).
- `coordinates` (*generator*): each pixel centre's position in the tile, `u` in red
  and `v` in green (blue 0, alpha 1), Blender's *Texture Coordinate*. Shape it with
  `math` (`sin`, `cos`, `atan2`, `divide`, …) and feed it to `sample_at` to remap a
  map by any per-pixel function: a perspective ground, a polar unwrap. It is a ramp,
  so it does not tile on its own.
- `curvature {radius, strength=1}` (*filter*, 1 input): height → `0.5 + strength × (h
  − mean of h on a ring radius away)`: above mid-grey on convex edges — edge wear —
  below in concave grooves — grime; `map_range` it into a hard mask.
- `directional_blur {length, angle=0}` (*filter*, 1 input): a motion blur, like
  Substance's *Directional Blur*: each pixel averages the input along a line `length`
  tiles long, centred on it and turned `angle` turns from the horizontal (0 smears
  left–right, 0.25 up–down, 0.125 along the diagonal). It wraps, so it tiles at **any**
  angle, which `mapping` cannot: brushed metal, rain streaks, scratches in several
  directions (one `directional_blur` of a `ridged` noise per direction). The samples
  are about a pixel apart, so a sub-pixel `length` still softens; `length` caps at one
  tile, and 0 passes the input through. It reads one bilinear sample per pixel of
  streak up to 64, and about `2√(length × longer side)` past that (two passes): 90
  for a one-tile streak at 2048², more above it. An estimate, not a measurement:
  roughly 10 s of one core for that streak at 2048², split across cores by rows.
- `directional_warp {strength, angle=0}` (*vector*, 2 inputs): pushes input 1 along
  `angle` (turns: 0 right, 0.25 down) by `strength × red` of input 2, in tiles, like
  Substance's *Directional Warp*: black moves nothing, white the full `strength`, and
  a negative strength pushes the other way (capped at ±16 tiles). The one-direction,
  grayscale counterpart of `warp`; wraps, so it tiles at any angle. Two reads a pixel.
- `distance {radius}` (*filter*, 1 input): mask → bevel. The input's red above 0.5 is
  inside, and each inside pixel reads its distance to the mask's edge divided by
  `radius`, clamped to 1; outside reads 0. It rounds **any** mask — a Voronoi cell, a
  thresholded noise, a stamp — the way `shape`'s `softness` and brick's `bevel` round
  their own; feed it to `bump_to_normal`. Straight, it is a chamfer; through `math`
  `func = "sqrt"` (a curve steepest at the rim and flat on top) it is a rounded,
  convex edge. Avoid an `ease` `color_ramp` here: it is flat at the rim too, so the
  edge reads as a pillow.
- `edge_detect {width, roundness=0}` (*filter*, 1 input): mask → its outline, like
  Substance's *Edge Detect*: red above 0.5 is inside (as `distance`), and a line
  `width` tiles wide straddles every edge. `roundness` 0 is a flat-topped line, 1 a
  round bead that bevels through `bump_to_normal`: grout from tiles, panel seams,
  crack rims. A pixel the line only partly covers reads the share it covers, so a
  line thinner than a pixel at a low `resolution` fades rather than vanishing, and
  the map's mean holds across resolutions. A mask with no edge reads black.
- `flood_bbox {output="size"|"center_x"|"center_y"}` (*filter*, 1 input:
  `flood_fill`'s): one flat grey per island from its bounding box, like Substance's
  *Flood Fill to BBox Size / Position*: `size` is the box's longer side in tiles,
  `center_x` / `center_y` where its centre sits, in `[0, 1)`; 0 off the islands. To
  select, scale or offset per piece. A box across the tile's edge is one box, its
  centre wrapped into the tile; on an axis an island loops round (a band across the
  tile) its size is 1 and its centre 0.5.
- `flood_fill {threshold=0.5}` (*filter*, 1 input): mask → its islands, like
  Substance's *Flood Fill*: the pixels whose red is above `threshold` (NaN is off),
  joined through their 8 neighbours and wrapping both axes, so a stone across the
  tile's edge or corner is one island (a checkerboard is one island too: its squares
  touch at the corners). Its output is what the `flood_*` readers take: alpha 1 on an
  island and 0 off it, which they relabel, so it survives any op that keeps alpha;
  RGB is a colour per island that only the preview reads (`Texture.Preview` shows
  each island tinted, the gaps black). A reader fed any other image reads its alpha
  the same way, so an opaque image is one island, the whole tile. Each `flood_*`
  node labels in one serial pass, about half a second of one core at 2048².
- `flood_gradient {angle=0, angle_jitter=0, seed=0}` (*filter*, 1 input:
  `flood_fill`'s): a 0→1 ramp across each island's bounding box, like Substance's
  *Flood Fill to Gradient*: along `angle` (turns; 0 runs left to right, 0.25 top to
  bottom), turned per island by up to ±`angle_jitter` turns; 0 off the islands.
  Tilted stones, slanted planks; a box across the tile's edge ramps as one. Each
  island's tilt is drawn like `flood_random`'s grey, so with `angle_jitter` above 0 a
  rolled mask or another resolution reshuffles the tilts; at 0 the ramps move with
  the mask. An island that loops round the tile on an axis (a band across it) ramps
  from the tile's edge and jumps back at the wrap: a ramp along a loop cannot tile.
- `flood_random {seed=0}` (*filter*, 1 input: `flood_fill`'s): one flat random grey
  per island, uniform in `[0, 1)`, 0 off the islands, like Substance's *Flood Fill to
  Random Grayscale*: per-stone colour (through a `color_ramp`), height or roughness.
  An island's grey comes from the seed and its identity: the cell of a 1/128-tile
  grid its centroid sits in, and its rank, in scan order, among the islands sharing
  that cell (densely packed specks, a ring and the stone at its centre, bands that
  loop the tile both ways), never from its number among all the islands. So every
  island draws its own grey, and adding an island in another cell leaves the others'
  greys alone. Moving the mask moves the centroids, so it reshuffles the greys, as in
  Substance. Across resolutions most islands keep their grey (about 80% of Voronoi
  stones from 128² to 1024²): one whose centroid sits near a grid line can land in
  the next cell and draw another.
- `gamma {gamma}` (*color*, 1 input).
- `gradient {kind="linear"|"linear_tiling"|"radial"}` (*generator*).
- `highpass {radius}` (*filter*, 1 input): `c − blur(c) + 0.5` per channel with a
  Gaussian of `radius` (a tile fraction, as `blur`): the detail smaller than the radius
  kept around mid-grey, the large shapes flattened, like Substance's *Highpass*. Lifts a
  detail layer off a photo or a busy noise before a normal map or roughness.
- `histogram_scan {position, contrast=0}` (*color*, 1 input): a threshold per channel,
  like Substance's *Histogram Scan*: `position` slides it from all black (0) to all
  white (1), `contrast` sets its edge from the input's own gradient (0) to a hard step
  (1); `position = 0.5` with `contrast = 0` passes the input through. It scans the
  **absolute** `[0, 1]` scale, not the input's own range (values outside it clamp
  first), so put `auto_levels` in front of a noise: a 4-octave fBM spans about
  0.35–0.64 and, straight in, bakes all black at `position = 0.25`, `contrast = 1`.
  `auto_levels` → `histogram_scan` turns a noise into a mask of a chosen coverage.
- `hue_sat_value {hue, sat, value}` (*color*, 1 input).
- `image {path, encoding="srgb"|"linear"}` (*generator*): an image file, see *Image
  files* below.
- `invert` (*color*, 1 input).
- `levels {in_low=0, in_high=1, gamma=1, out_low=0, out_high=1}` (*color*, 1 input):
  per channel, `[in_low, in_high]` → `[0, 1]` (clamped), raised to `1 / gamma`, then
  onto `[out_low, out_high]`, like Substance's *Levels*: `gamma` above 1 lifts the
  midtones and below 1 sinks them without moving the ends.
- `map_range {from_min, from_max, to_min, to_max}` (*math*, 1 input).
- `mapping {scale=[1,1], rotation=0, translation=[0,0], tiling=true}` (*vector*, 1
  input): bilinear, wrapped resample; `rotation` in turns; each transform left out is
  the identity, so `mapping {rotation = 0.25}` only turns. With `tiling` the rotation
  snaps to a quarter turn (*Tiling*), so a stretched noise only ever lines up with the
  axes: `rotation = 0.125` on a `scale = {1, 12}` stretch gives vertical lines, not
  diagonal ones. For streaks at any angle, `directional_blur` a noise (or
  `directional_warp` it).
- `math {func, value=0.5}` (*math*, 1 or 2 inputs) with `func` ∈
  `add/subtract/multiply/divide/power/min/max/abs/fract/sqrt/sin/cos/atan2`: the
  right-hand side is `value`, or with a second input wired, that input's matching
  channel per pixel (`value` is then ignored, Blender's Math with both sockets
  linked). `abs`, `fract`, `sqrt`, `sin` and `cos` have no right-hand side, so they
  take exactly 1 input. The trig funcs work in **radians**, as Blender's: `sin` of a
  `coordinates` channel times `2π` is one cycle per tile, and `atan2` is the angle of
  the point (right-hand side, channel), `-π` to `π`.
- `mix {mode, factor=1}` (*color*, 2 or 3 inputs) with `mode` ∈
  `mix/add/multiply/screen/overlay/subtract/difference/lighten/darken/divide`: inputs
  `{a, b, mask}`; the result is `a` blended toward `mode(a, b)` by `factor`, and an
  optional third input `mask` makes that per pixel (`factor × mask.red`; black keeps
  `a`, white takes the full blend). `lighten` is the per-channel max and `darken` the
  min — the way to combine height fields (rivets over a panel, stones over mortar),
  where `add` overshoots; `divide` is `a / b`, and a channel of `b` at (near) 0 leaves
  `a`.
- `noise {kind="perlin"|"fbm"|"ridged"|"turbulence", scale, octaves=1, lacunarity=2,
  gain=0.5, distortion=0, seed=0}` (*generator*): `ridged` is `1 − |n|` squared — rock
  veins, scratches; `turbulence` is `|n|` — smoke, grime; the fBM family stacks
  `octaves`, each `lacunarity`× the frequency and `gain`× the amplitude of the last;
  `perlin` is one octave. `distortion` is Blender's Noise Texture *Distortion*: a
  domain warp built in, pushing each point by up to about that many lattice cells
  along two more tiling noises before reading it (swirled smoke, marbled stone)
  without a `warp` chain; it still tiles, and costs about 0.5 s of one core at
  2048² over the noise itself. Capped at ±1000; a NaN reads 0.
- `normal_blend {strength=1}` (*vector*, 2 inputs: base, detail): two normal maps, as
  `bump_to_normal` writes them, combined by Reoriented Normal Mapping: the base is
  turned by the detail's tilt, so scratches over a panel follow the panel's bevel.
  `strength` scales the detail's tilt, 0 leaving the base; a flat normal on either
  side returns the other. Never `mix` normal maps: adding or averaging their RGB
  flattens them.
- `rgb_to_bw` (*math*, 1 input).
- `sample_at` (*vector*, 2 or 3 inputs): input A read at the domain point the other
  inputs name, in tile units and wrapped: with two inputs B's red is `u` and its
  green `v` (what `coordinates` writes), with three B's red is `u` and C's red is
  `v`. Each pixel is filtered by its footprint (how far the coordinates move to its
  neighbours) through a mip chain of A, trilinear, so a remap that shrinks A reads
  its average instead of aliasing. A jump in the coordinates (a `fract` wrap) is not
  taken as a footprint, so it stays sharp. `sample_at` of `coordinates` is A
  unchanged.
- `separate_rgb {channel=0..3}` (*vector*, 1 input).
- `shape {kind="circle"|"rect"|"rounded_rect"|"line"|"polygon"|"star", size=[w,h],
  roundness=0.25, softness=0, sides=6, points=5, inner_radius=0.5,
  profile="flat"|"cone"|"paraboloid"|"hemisphere"}` (*generator*): one shape centred
  in the tile, 1 inside and 0 outside: `circle` is the ellipse filling `size`, `line`
  a horizontal stroke `w` long and `h` thick with round caps — turn it with `mapping
  {rotation = 0.25}`; `polygon` is a regular polygon of `sides` (3–64) and `star` one
  of `points` (3–64) with its inner vertices at `inner_radius` (0–1) of the outer,
  both inscribed in the ellipse of `size` with a vertex straight up (hex bolts,
  rivets, stars); `roundness` 0–1 rounds a `rounded_rect`'s corners; `softness > 0`
  ramps the edge from 0 to 1 that far inside, a bevel profile for `bump_to_normal`.
  `profile` turns any shape into a ready height stamp, Substance's *Shape* heights:
  `flat` is the mask, the others rise from 0 at the edge to 1 at the deepest point
  (the centre): `cone` straight (a cone on a circle, a pyramid on a square `rect`),
  `paraboloid` round on top, `hemisphere` a dome. On a `circle` the height follows
  the ellipse's own stretch, so an elongated one is one dome peaking at the centre;
  a non-square `rect` rises to the inside distance of its short side, so it gets a
  hip roof with a ridge along its long axis, not one peak. Feed one to `tile` →
  `bump_to_normal` for studs, rivets, bolts and pebbles; `softness` applies to `flat`
  only. A polygon or star with a NaN `size` reads 0. Cost: about 0.3 s of one core at
  2048² for a star; one larger than the tile unions its neighbours' copies (9 for
  up to 3 tiles across, about 2 s; at most 81).
- `sharpen {intensity=1, radius=0.004}` (*filter*, 1 input): an unsharp mask,
  `c + intensity × (c − blur(c))` with a Gaussian of `radius` (a tile fraction, about
  two pixels at 512²): crisps an `image` input or a blurred chain. Unclamped, so a
  hard edge overshoots until the bake clamps it.
- `slope_blur {intensity, samples=8, mode="blur"|"min"|"max"}` (*filter*, 2 inputs:
  image, slope): smears input 1 down the slopes of input 2's red, like Substance's
  *Slope Blur*: each pixel reads itself and `samples` points (1–64) beyond it along
  `intensity ×` the slope's gradient (height per tile, so a reach in tiles), uphill of
  it, and keeps their mean (`blur`), darkest (`min`) or brightest (`max`). The
  gradient is taken over 1/64 of a tile, so a hard edge (a `shape` with `softness =
  0`) reads as a slope of 64 over a band 1/64 tile wide at any resolution. Flat ground
  is left as it is; a negative intensity smears up the slopes. Erosion, chipped paint edges, grime
  running off a shape: a noise through `slope_blur` on a `shape`'s height runs down its
  sides. The reach caps at one tile. `samples + 5` reads a pixel: an estimate of
  about 1–2 s of one core at 2048² with 8 samples.
- `tile {count, jitter=0, rotation_jitter=0, scale_jitter=0, value_jitter=0,
  probability=1, output="stamp"|"random"|"index", blend="max"|"add"|"over", seed=0}`
  (*vector*, 1–3 inputs: the stamp, then an optional mask, then an optional size
  map): Substance Designer's *Tile Sampler* essentials. Stamps input 1 once per cell
  of a `count`×`count` grid; each cell is seeded from the recipe and node `seed` with
  an offset of up to ±`jitter`/2 cells, a turn of up to ±`rotation_jitter`/2 turns
  and a scale in `1 ± scale_jitter/2`, each jitter clamped to 0–1. The input is read
  as **one stamp** — the unit square, nothing outside it — so give it empty margins
  (a `shape` smaller than the tile); a stamp pushed over its cell's border is drawn
  into the neighbour cell. `output = "random"` fills each stamp with the input's red
  times its own seeded grey (per-rivet tint, per-pebble height); `"index"` with the
  red times the cell's row-major index `(i + 1) / count²`. `value_jitter` (0–2)
  multiplies each stamp by `1 ± value_jitter/2`. `probability` (0–1) is the chance a
  cell gets a stamp. Input 2, the mask: no stamp in a cell whose centre reads below
  0.5 red. Input 3, the size map: the red at the cell's centre scales that stamp
  (0–2; 0 or less draws none); to use a size map with no mask, wire a white
  `constant` as input 2. Both are read at the cell's centre, wrapped, so they don't
  vary with resolution. Placement draws never change when `probability` or
  `value_jitter` are added: each takes its own hash stream. Overlaps: `max` keeps the
  brighter value per channel, `add` sums the colours, `over` lays each stamp on the
  ones before it, premultiplied: `out = colour × alpha + out × (1 − coverage)`, in
  row-major order of the cells (row 0 first, left to right), whatever the band
  split. `over` reads a stamp's **brightness as its coverage** (its red under
  `output = "random"` or `"index"`, its brightest channel under `"stamp"`, times its
  alpha), so a soft edge stays soft over what is under it and a dark colour stamp is
  thin; output alpha is left as `max` leaves it. A NaN `probability` or
  `value_jitter` is ignored, an out-of-range one clamped. Rivets are `shape circle`
  → `tile`.
- `voronoi {scale, output="distance"|"cells"|"f2"|"edges"|"smooth_f1",
  metric="euclidean"|"manhattan"|"chebyshev", randomness=1, smoothness=1, seed=0}` (*generator*):
  `f2` is the distance to the second-nearest point, `edges` is F2 − F1 — ~0 on cell
  borders: cracks, dried mud, cobble outlines; `metric` is how distance is measured:
  `euclidean` gives round cells, `manhattan` (`|dx| + |dy|`) diamond-cut ones,
  `chebyshev` (`max(|dx|, |dy|)`) blocky, straight-edged ones — stones, tiles,
  flagstones; `randomness` 0 is a regular grid, 1 full jitter. `smooth_f1` is
  Blender's *Smooth F1*: F1 with its creases rounded over `smoothness` (0–1, default
  1; the other outputs ignore it) — rounded, blobby cells for pebbles, scales and
  dried paint, and the soft minimum the hard F1 lacks; it searches two rings of
  cells, so it costs about 2 s of one core at 2048², against 0.6 s for `distance`. `smoothness`
  0 is F1; a NaN reads 0.
- `warp {strength}` (*vector*, 2 inputs): samples input 1 displaced by input 2's R/G,
  centred on 0.5 so mid-grey moves nothing; `strength` is in tile units; feed input 2
  a `combine_rgb` of two different noises — a grayscale one has R = G and only pushes
  along the diagonal.
- `wave {kind="bands"|"rings", frequency, profile="sine"|"saw"|"triangle",
  distortion=0, detail=2, seed=0}` (*generator*): `frequency` cycles across the tile,
  as Blender's *Wave Texture*: `saw` ramps up and drops (planks, shingles, terraces),
  `triangle` rises and falls in straight lines (corrugation, ridges). `distortion`
  pushes the phase by a tiling fBM of `detail + 1` octaves (`detail` 0–15) at the
  wave's own frequency, in radians of phase as Blender's: wood rings and marble with
  no `warp` chain. It costs about 0.85 s of one core at 2048² with `detail = 2`; 0 costs
  nothing.
  Capped at ±1000; a NaN reads 0.
- `white_noise {seed=0}` (*generator*).

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

<!-- sorted: one bullet per param, so two branches adding linkable params land on different lines -->
- `blur.radius`: read at each pixel (see below).
- `bright_contrast.bright`, `bright_contrast.contrast`: read at each pixel.
- `bump_to_normal.strength`: read at each pixel.
- `curvature.strength`: read at each pixel.
- `distance.radius`: read at each pixel.
- `gamma.gamma`: read at each pixel.
- `histogram_scan.position`, `histogram_scan.contrast`: read at each pixel.
- `hue_sat_value.hue`, `hue_sat_value.sat`, `hue_sat_value.value`: read at each pixel.
- `noise.gain`: read at each pixel.
- `normal_blend.strength`: read at each pixel.
- `shape.softness`: read at each pixel.
- `tile.jitter`, `tile.rotation_jitter`, `tile.scale_jitter`: read once per cell.
- `voronoi.randomness`: read once per cell.
- `warp.strength`: read at each pixel.

A param read **once per cell** is read at the cell's centre, wrapped, so each cell
keeps one stamp or one point and the result still tiles. A blur's radius varies
smoothly: sharp where the link is dark, soft where it is bright, each pixel averaging
its own window in constant time whatever its width. A Gaussian is still three
passes, and spreads as far as the same radius as a number. A linked box is not
rounded to a whole pixel as a number's box is (a rounded radius would step visibly
across the map), so it matches the number only at whole-pixel radii.

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
local damaged = { resolution = 256, output = "tiles", nodes = {
  { id = "tile",   op = "shape", kind = "rounded_rect", size = {0.9, 0.9}, softness = 0.1 },
  { id = "damage", op = "noise", kind = "fbm", scale = 3, octaves = 4, seed = 5 },
  { id = "edge",   op = "map_range", from_min = 0.55, from_max = 0.65, to_min = 0, to_max = 1,
    inputs = {"damage"} },
  { id = "mask",   op = "clamp", min = 0, max = 1, inputs = {"edge"} },
  { id = "tiles",  op = "tile", count = 8, inputs = {"tile"},
    jitter          = { node = "mask", max = 0.6 },
    rotation_jitter = { node = "mask", max = 0.15 },
    scale_jitter    = { node = "mask", max = 0.3 } },
} }
local stats = Texture.Stats(damaged)  -- `mask` is listed with the rest
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

### Graphs — a recipe as one node

A recipe can use another recipe as **one node**, as Substance Designer instances a
graph and Blender a node group: build "worn brick" once, then drop it into any
material with its `age`, `tint` and `mortar_width` knobs.

**The file.** A graph is a recipe saved as `<name>.graph.json` (the
`Texture.ToJson` form), anywhere in the project, named by its path from the project
root as an `image` node names its file (outside the project it is read, with a
once-per-file warning). The extension is what makes it a graph: instancing any other
file is refused. Every recipe may be one; two optional fields give it an interface:

```json
{
  "resolution": 256,
  "params": {
    "age":  { "type": "number", "default": 0.3, "min": 0, "max": 1 },
    "tint": { "type": "color",  "default": "#8a4b32" }
  },
  "inputs": ["mask"],
  "nodes": [
    { "id": "mortar", "op": "brick", "rows": 8, "cols": 4 },
    { "id": "wear", "op": "noise", "kind": "fbm", "scale": 6, "gain": { "param": "age" } },
    { "id": "color", "op": "constant", "color": { "param": "tint" } },
    { "id": "out", "op": "mix", "mode": "multiply", "inputs": ["color", "mortar", "mask"] }
  ],
  "output": "out",
  "outputs": { "height": "wear" }
}
```

- **`params`**: name → `{ type, default, min?, max? }`, `type` one of `number`,
  `integer`, `bool`, `color`, `text` (`min`/`max` for the first two). Any node value
  may be the placeholder `{ param = "age" }`, a whole param (a number, a colour, an
  enum or an `image` node's path as `text`, a seed), whether the param takes a link
  or not. An instance's own `path` is the one value that takes none, and is refused:
  the graph files are read before any param has a value. A recipe
  with `params` baked on its own bakes at the defaults, and `Texture.ToJson` writes
  the placeholders back.
- **`inputs`**: an ordered list of names inner nodes read like node ids (in
  `inputs` and in links). A graph with inputs bakes only through an instance.
- **Outputs** are the recipe's own `output` (or its last node) and `outputs`. An
  output names a node, never an input.

**The instance.**

```lua no-run
{ id = "brick", op = "graph", path = "textures/brick.graph.json",
  params = { age = 0.5 }, seed = 3, inputs = { "damage" } }
```

- `params` sets any of the graph's params; the rest keep their defaults. A value of
  the wrong type, outside `min`/`max`, or naming no declared param is an error naming
  it (rusty refuses where Blender clamps). Inside a graph, an instance's `params` may
  pass the outer graph's on: `params = { age = { param = "wear" } }`.
- `inputs` wires the graph's `inputs` in order, exactly as many as it declares.
- Downstream, `brick` reads the graph's `output` and `brick.height` its named output
  `height`: in `inputs`, in a link's `node`, and in the recipe's `output`/`outputs`.
- `seed` (default 0) folds into every inner node's seed. 0 leaves them as written, so
  the instance bakes byte-identically to its nodes pasted by hand; any other seed
  changes every stochastic inner node, unseeded ones included, so two instances of
  one graph differ. The recipe `seed`, `resolution` and `size` are the outer
  recipe's; the graph's own are used only when it bakes alone.

**What is stored is the instance.** `Texture.ToJson` and a material's `maps` keep the
`op = "graph"` node, never its expansion, so editing the graph file changes every
recipe that uses it on its next bake (`Material.Rebake` included). Each verb reads the
graph files when it runs, nested ones too, and expands every instance into one flat
graph before anything is checked: inner ids become `brick/<inner id>` (nested,
`brick/wear/blur_2`), and `Texture.Stats` and `Texture.Preview` show the inner nodes
under those ids.

**Errors** name the nesting path and the file: `node "brick/wear/blur_2" (blur):
references unknown input "brick/wear/nowhere" (in textures/wear.graph.json)`. A graph
reached twice on one chain of instances is refused with the chain
(`a.graph.json → b.graph.json → a.graph.json`), instances nest at most 16 deep, and
an expansion holds at most 65 536 nodes (past that it is refused, naming the chain of
graphs it was in). Two nodes sharing an id in one recipe or one graph are refused,
an instance's id included, and every error from inside an instance names its graph
file.

**`Texture.Ops()`** lists the project's graphs after the ops, each as `op = "graph"`
with its `path`, `category = "graph"`, `min_inputs` = `max_inputs` = its input count,
its `inputs` names and its `params` (`name`, `type`, `default`, `min`, `max`). A graph
file that does not parse is left out, with a warning naming it.

### Tiling

Every periodic param is a **count of repeats per tile**, rounded at evaluation (the
field stays a number, so `4.5` is accepted — it bakes as `5`), and capped at 2²⁰
(*Units*):

| Op | Param | Rounds to |
|---|---|---|
| noise, voronoi | `scale` | nearest whole number ≥ 1 |
| noise (fBM family) | each octave's `scale × lacunarity^octave` | nearest whole number ≥ 1 (so a fractional `lacunarity` still tiles) |
| wave | `frequency` | nearest whole number ≥ 1 |
| anisotropic_noise | `scale`, and `scale ÷ anisotropy` along the fibres | nearest whole number ≥ 1, at most 4096, per lattice direction (at least 2 along on an axis) |
| anisotropic_noise | `angle` | the nearest tiling direction, see its bullet |
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
does `shape` (a shape larger than the tile is clipped at the edge, symmetrically;
a `polygon` or `star`, which an odd count leaves lopsided top to bottom, is instead
joined to its copies in the neighbouring tiles, so it tiles).
`flood_gradient` has a seam on one kind of island: one that loops round the tile on
an axis (a band across it) has no edge there, so its ramp runs from the tile's edge
and jumps back at the wrap; every other island ramps across the edge as one.
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
- `directional_blur.length` and `directional_warp.strength` are in tiles; `angle`
  on both is in turns, like `mapping.rotation`, but never snaps: they tile at any
  angle. `slope_blur`'s reach is `intensity ×` the slope in height per tile, in tiles.
- `ambient_occlusion.radius` and `.depth` are in tiles; `edge_detect.width` is the
  line's whole width in tiles; `highpass.radius` and `sharpen.radius` are tile
  fractions like `blur.radius`.
- `flood_gradient.angle` and `.angle_jitter` are in turns; `flood_bbox` writes its
  size and centre in tiles.
- `noise.distortion` is in lattice cells (one cell is `1 / scale` of the tile);
  `wave.distortion` in radians of phase (`2π` is one cycle); `anisotropic_noise.angle`
  in turns, like `mapping.rotation`; `shape.inner_radius` a fraction of the outer.
- Every whole count in *Tiling* (a `scale`, `frequency`, `count`, an octave's
  period) is capped at 2²⁰ = 1 048 576 per tile, far past one cell per pixel at the
  largest bake; a NaN reads 1, so `1e30`, an infinity or a NaN bakes, never panics.
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
