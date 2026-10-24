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
| `Assets.Refresh` | `()` | `{ converted = { wavPath, ... }, skipped = { { path, reason }, ... } }` — imports what arrived (below) |
| `Assets.GetLightmapUVSettings` | `(path)` | `{ generate, hardAngle, packMargin }` — a model's Generate Lightmap UVs setting (below) |
| `Assets.SetLightmapUVSettings` | `(path, { generate?, hardAngle?, packMargin? })` | how many of the scene's meshes it re-imported |
| `Assets.GetTextureImportSettings` | `(path)` | `{ wrapMode, filterMode, generateMipmaps }` — an image's import settings (below) |
| `Assets.SetTextureImportSettings` | `(path, { wrapMode?, filterMode?, generateMipmaps? })` | nothing |

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

An MP3 is left in place, and listed in `skipped` with the reason, when its `.wav`
sibling already exists (a refresh never overwrites a file) or when it does not decode
(a file still being copied in; the next refresh retries it). An `AudioSource` or
`PlayAt` that still names `foo.mp3` after conversion plays `foo.wav`.

```lua no-run
local r = Assets.Refresh()
for _, wav in ipairs(r.converted) do print("imported " .. wav) end
for _, s in ipairs(r.skipped) do print(s.path .. ": " .. s.reason) end
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
Settings** card: how a texture is sampled, stored per image in its `<file>.meta`
sidecar. Each material map (albedo, metallic, roughness, normal, emissive, a
shader's `mask`) samples through its own texture's settings, and so do particle and
ribbon textures. UI images do not yet: they keep the UI's own clamped, linear
sampler. Nor do decals: a decal's maps are copied into the one decal atlas, which
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

`SetTextureImportSettings` changes only the keys it is given, writes the sidecar (a
file with none gets one), and the texture is re-uploaded with the new settings on the
next frame — the same verb the inspector's card runs. An unknown mode name is an
error naming the known ones. A texture extracted from a converted model
(`model.fbx#tex/Skin`) keeps the defaults.

```lua no-run
Assets.SetTextureImportSettings("project/textures/pixel_font.png", { filterMode = "Point" })
print(Assets.GetTextureImportSettings("project/textures/pixel_font.png").wrapMode)  -- Repeat
```
