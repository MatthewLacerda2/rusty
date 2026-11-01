## `Assets`

The project's importable assets — the "see what I can place" half of authoring.
`Assets.Manifest()` (alias `Assets.List()`) walks the open project's `assets/` folder, imports
every model file (`.gltf`/`.glb`/`.obj`/`.fbx`/`.blend`; an `.fbx` or a `.blend` is read through the glTF it
is converted to and cached in `assets/generated/imported/`, which the walk skips), and returns a structured catalogue of the
addressable sub-objects inside each file plus their footprint (so the agent can lay
things out without overlap) and material count. Unlike the rest of the surface,
this returns a Lua **table** (not a scalar triple): the result is nested data.

Each `subObject`'s `reference` is the canonical `path::sub_object` string and
round-trips with `AssetRef` / `import_sub_mesh`, so it can be handed straight to the
mesh-instantiation path to place exactly what the manifest names. A sub-object name
may hold `::` (the reference splits on its first `::`); a file whose path holds `::`
can't be referenced, so it is skipped with a warning naming why (rusty's rule; see
`Scene.Instantiate` in [`Scene`](Scene.md)). Files that fail to import are skipped; a missing root yields an empty list. Output is deterministic
(files sorted by path, sub-objects in source order). During Play nothing is converted
(#1253), so an `.fbx` or a `.blend` is listed from its last import: the one made as Play began, or
before a source changed during Play. An exported project (`make export`, #1267)
has the cache but not the sources, so it lists no `.fbx` or `.blend`; instancing
one by its source path still works.

| Function | Signature | Returns |
|---|---|---|
| `Assets.Manifest` | `()` | array of asset tables (see shape below) |
| `Assets.List` | `()` | alias for `Assets.Manifest` |
| `Assets.Refresh` | `()` | `{ converted = { wavPath, ... }, skipped = { { path, reason }, ... }, compressed, failed = { path, ... } }` — imports what arrived (below); images wait for Stop |
| `Assets.GetLightmapUVSettings` | `(path)` | `{ generate, hardAngle, packMargin }` — a model's Generate Lightmap UVs setting (below) |
| `Assets.SetLightmapUVSettings` | `(path, { generate?, hardAngle?, packMargin? })` | how many of the scene's meshes it re-imported |
| `Assets.GetModelImportSettings` | `(path)` | `{ scaleFactor }` — a model's Scale Factor (below) |
| `Assets.SetModelImportSettings` | `(path, { scaleFactor? })` | how many of the scene's meshes it re-imported |
| `Assets.GetTextureImportSettings` | `(path)` | `{ wrapMode, filterMode, generateMipmaps, textureType, sRGB, compression, maxSize }` — an image's import settings (below) |
| `Assets.SetTextureImportSettings` | `(path, { wrapMode?, filterMode?, generateMipmaps?, textureType?, sRGB?, compression?, maxSize? })` | nothing |

Returned shape (Lua, 1-indexed arrays):

```lua
{
  {
    path = "assets/models/crates.glb",
    materialCount = 2,            -- materials in the file's shared table
    subObjects = {
      {
        id = "Sedan",
        reference = "assets/models/crates.glb::Sedan",  -- round-trips with AssetRef
        materialCount = 1,         -- distinct materials its runs use (0 when none)
        size = { x = 4.2, y = 1.5, z = 1.8 },            -- AABB extent (max - min)
        min  = { x = -2.1, y = 0.0, z = -0.9 },          -- absent if the mesh is empty
        max  = { x =  2.1, y = 1.5, z =  0.9 },
      },
    },
  },
}
```

### `Assets.Refresh`

Unity's `AssetDatabase.Refresh()`: import whatever arrived in the project's `assets/` tree since
the last look. Today that is audio: every `.mp3` anywhere under `assets/` is decoded
once, its encoder delay and padding trimmed, written as a 16-bit PCM `.wav` beside it
(same name, same channels and sample rate), and the `.mp3` is removed, so a built game
carries WAV, never MP3. OGG is never converted. The editor runs the same refresh at
boot and whenever its window regains focus, logging each conversion to the console;
call it yourself after writing or copying an `.mp3` into the project.

It also makes every image's compressed copy current (see `compression`, below) and
returns how many it made in `compressed`; each image that fails (it does not decode,
say) is warned of in the console, listed in `failed` by path, and draws
uncompressed. It waits for them, on any build: the way to compress textures from a
script or a session in edit mode. During Play it still imports audio, as Unity's
`AssetDatabase.Refresh` imports in Play, but compresses no image (`compressed` is
`0`): images wait for Stop, as a model's conversion does.

An MP3 is left in place, and listed in `skipped` with the reason, when its `.wav`
sibling already exists (a refresh never overwrites a file) or when it does not decode
(a file still being copied in; the next refresh retries it). An `AudioSource` or
`PlayAt` that still names `foo.mp3` after conversion plays `foo.wav`.

```lua no-run
local r = Assets.Refresh()
for _, wav in ipairs(r.converted) do print("imported " .. wav) end
for _, s in ipairs(r.skipped) do print(s.path .. ": " .. s.reason) end
```

### Model import settings

Unity's model-import **Scale Factor** (#1398), and the model inspector's *Scale Factor*
drag: one uniform scale, stored per model in the `<file>.meta` sidecar (default `1`), that
the importer applies to the model's data itself — its vertices, its skeleton's bind pose
and its animations' translation keys. Everything downstream sees the scaled model: the
mesh, its mesh collider, a nav bake, the lightmap unwrap, `Assets.Manifest()`'s footprint
and the bone GameObjects. Use it for a model authored at the wrong unit (centimetres, say);
an `.fbx` file's own unit is already converted on import, so this is the per-model
override on top. It is not the entity's `Transform` scale, which stays `1`.

`SetModelImportSettings` changes only the keys it is given, writes the sidecar, and
re-imports every mesh in the open scene instanced from that model, a skinned one with its
bones moved to the scaled rest pose (a bone moved off its rest pose keeps its pose) — the
same verb the inspector's drag runs. `scaleFactor` must be finite: a NaN or infinite one
changes nothing, and one at or below `0.0001` is floored there. A factor the sidecar
already holds (or none given) changes nothing either: no write, no re-import.

An LODGroup over those meshes has its `size` scaled with them, as a convenience for the
open scene: a size still at its LOD0 meshes' extent follows the new extent, one set by
hand is scaled by as much. The size is an authored, saved value, as Unity's LODGroup
size is, so another scene's groups keep theirs (Unity's *Recalculate Bounds* is the
manual step there too).

A whole model already spawned (`Scene.Instantiate(modelPath)`, one child per node) is a
linked instance of the model (#1637), so it is rebuilt from the scaled import: its parts'
offsets scale with the meshes, in the open scene at once and in every other scene when it
loads, and a part moved by hand keeps its own place (an override). A changed source file
reaches it the same way.

```lua no-run
Assets.SetModelImportSettings("project/models/crate_cm.glb", { scaleFactor = 0.01 })
print(Assets.GetModelImportSettings("project/models/crate_cm.glb").scaleFactor)  -- 0.0099999997764826: an f32
```

### Generate Lightmap UVs

Unity's model-import checkbox of the same name (#831), and the model inspector's
**Generate Lightmap UVs** box: the importer unwraps a non-overlapping second UV set, so
a mesh exported without one (no glTF `TEXCOORD_1`) still gets a lightmap from
[`Lighting.BakeLightmaps()`](Lighting.md#lightmaps). It is stored per model in the
`<file>.meta` sidecar, off by default as in Unity. When on, the generated unwrap
**replaces** an authored `TEXCOORD_1`; when off, an authored one is used as-is.

- `generate` — the checkbox.
- `hardAngle` — degrees, 0–180, default `88`: faces meeting at a sharper angle than this
  are split into separate charts (Unity's *Hard Angle*).
- `packMargin` — texels between charts, 1–64, default `4`, measured at the bake's default
  `texelsPerUnit` (8), so it holds whatever the mesh's size; a bake at a higher
  resolution gets proportionally more (Unity's *Pack Margin*).

`SetLightmapUVSettings` changes only the keys it is given (each clamped to its range),
writes the sidecar, and re-imports every mesh in the open scene instanced from that
model, so the change shows at once — the same verb the inspector's box runs. A file
with no sidecar yet gets one. Skinned meshes are never unwrapped (they are never
lightmapped). The unwrap is deterministic: the same mesh and settings always give the
same UVs, so a bake stays byte-identical across machines.

```lua no-run
Assets.SetLightmapUVSettings("project/levels/warehouse.glb", { generate = true })
print(Assets.GetLightmapUVSettings("project/levels/warehouse.glb").hardAngle)  -- 88
Lighting.BakeLightmaps()
```

### Texture import settings

Unity's Texture Import Settings (#1323), and the image inspector's **Texture Import
Settings** card: how a texture is sampled, compressed and sized, stored per image in its `<file>.meta`
sidecar. Each material map (albedo, metallic, roughness, normal, emissive, a
shader's `mask`) samples through its own texture's settings, and so do particle and
ribbon textures. A UI Image takes its texture's filter and mips but always clamps,
as Unity imports a UI sprite, so its edge never bleeds the opposite side into its
border (a `Point` image draws crisp pixel art). Decals do not: a decal's maps are copied into the one decal atlas, which
always builds a full mip chain and samples every decal through one trilinear,
clamped sampler.

- `wrapMode` — how UVs outside 0–1 read it: `"Repeat"` (tile, the default),
  `"Clamp"` (stretch the edge texels) or `"Mirror"` (tile, flipping every other copy).
  Unity's `MirrorOnce` is left out: the GPU layer has no such mode.
- `filterMode` — `"Point"` (nearest texel, pixelated), `"Bilinear"` (smooth within a
  mip, snapping between mips) or `"Trilinear"` (smooth between mips too, and
  anisotropic; the default).
- `generateMipmaps` — upload a full mip chain (on by default, as in Unity). Off, only
  the full-size image is uploaded, so a tiled surface far away shimmers.

- `textureType` — what the texels hold (#1574), Unity's *Texture Type*; it picks the
  compressed format, so the import can make it before anything draws:

  | type | format | for |
  |---|---|---|
  | `"Default"` | BC7 (a byte a texel), alpha kept, mips averaged in linear light (raw with `sRGB` off) | colour, packed data (glTF's occlusion-roughness-metallic, with `sRGB` off) |
  | `"NormalMap"` | BC5 (a byte a texel): x and y; the shader rebuilds z | tangent-space normal maps |
  | `"SingleChannel"` | BC4 (half a byte): red only | roughness, metallic, masks |

  Assigning a `"Default"` image to a material's normal slot (`Material.SetNormalMap`,
  a material document, a maps recipe, the inspector) sets it to `"NormalMap"` and
  says so in the console, Unity's *Fix now* done for you; never during Play. A
  `"Default"` texture is sampled as sRGB colour in a colour slot and raw in a data
  slot. A texture drawn in a slot its type cannot serve (a normal map as albedo, a
  single-channel map as colour) is uploaded uncompressed for that use, with one
  console warning, so it never loses a channel.
- `sRGB` — Unity's *sRGB (Color Texture)*, read for a `"Default"` texture only (the
  inspector shows it then): on (the default), its texels are sRGB colour, mips and
  a `maxSize` downscale average in linear light; off, they are data stored raw (a
  packed metallic/roughness map), averaged and sampled as stored even in a colour
  slot. Assigning a `"Default"` image to a material's metallic or roughness slot
  (`Material.SetMetallicMap`/`SetRoughnessMap`, a material document, a maps recipe,
  the inspector, a model's import) turns it off and says so in the console; never
  during Play. `"NormalMap"` and `"SingleChannel"` are always data.
- `compression` — how it is block-compressed on the GPU (#1574), Unity's
  *Compression*: `"None"` (RGBA8, four bytes a texel), `"Normal"` (the default) or
  `"HighQuality"`, which encodes BC7 slower and closer to the source. Unity's
  `Normal` picks BC1 (half a byte) for an opaque colour map; rusty does not, because
  the pure-Rust BC1 encoders flatten a block of two contrasting colours (red beside
  blue) to one. The blocks and every mip are made at import, never at a draw, into
  the project's import cache (`assets/generated/imported/<path>.bctex`), and made
  again only when the image, its type, `sRGB`, compression, max size or mipmaps, or
  the encoder change. The editor makes them in the background when an image is added,
  its settings change or the project opens (the console says each one); entering
  Play waits for any still stale, as it waits for models; an export makes and ships
  them. The standalone player and the `play` harness never encode on their own:
  they play from the copies the editor, an export or `Assets.Refresh()` (in edit
  mode) made. An image that fails to import is warned of and draws, and ships,
  uncompressed. Until its copy is made, a texture draws
  uncompressed. Encoding BC7 is slow, a few seconds for a 2048² map spread over
  every core (half of them in the editor's background, so it stays responsive), so a project's first import of many large maps takes minutes, once. An
  image whose (fitted) sides are not multiples of 4 is uploaded uncompressed, as
  Unity leaves it; a downscale to `maxSize` rounds its sides to multiples of 4. On a
  GPU without BC compression every texture uploads uncompressed, with one console
  warning. `Debug.Stats().texture_bytes` is what the textures take on the GPU.
- `maxSize` — the longest side it is uploaded at (#1574), Unity's *Max Size*: one of
  32, 64, 128, 256, 512, 1024, 2048 (the default), 4096 or 8192. A larger image is
  downscaled at import, its aspect kept (colour in linear light); the file on disk
  is never changed. A UI image still measures its slices and tiling in the source
  image's pixels. Another number is an error naming the list; a sidecar holding one
  is warned of once and read as 2048.

`SetTextureImportSettings` changes only the keys it is given, writes the sidecar (a
file with none gets one), and the texture is re-uploaded with the new settings on the
next frame — the same verb the inspector's card runs. An unknown mode name is an
error naming the known ones. A texture extracted from a converted model
(`model.fbx#tex/Skin`) keeps the defaults.

```lua no-run
Assets.SetTextureImportSettings("project/textures/pixel_font.png", { filterMode = "Point", compression = "None" })
Assets.SetTextureImportSettings("project/textures/floor_albedo.png", { compression = "HighQuality", maxSize = 1024 })
Assets.SetTextureImportSettings("project/textures/floor_rough.png", { textureType = "SingleChannel" })
print(Assets.GetTextureImportSettings("project/textures/pixel_font.png").wrapMode)  -- Repeat
```
