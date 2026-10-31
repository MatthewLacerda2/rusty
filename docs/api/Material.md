## `Material`

A *material* is a reusable **asset** — glTF 2.0 metallic-roughness data — stored
once in the scene's per-World **material library**; an entity carries only a thin
**reference** to one by name (its `MaterialComponent`). Many entities can share a
single material, so editing it once updates all of them. A mesh with several
materials (an imported model whose faces use more than one) has one material
**slot** per material run, each referencing its own library material (see
[*Material slots*](#material-slots) below); the setters below act on the **first**
slot, Unity's `renderer.sharedMaterial`. These functions resolve
the entity's referenced material in the library and mutate that shared asset;
calling one on an entity that has no material yet **creates** a default library
material and attaches the reference. `SetTexture` sets the albedo (`base_color`)
map; an empty path clears it. Any map path may be `"rt:<name>"` — a camera's render
texture ([`Camera`](Camera.md#camera-entities-projection-and-render-textures-430)),
the in-world security monitor: put it on the emissive map so the screen glows its
picture whatever the lighting. A map may also be `"<model>#tex/<name>"`, a texture
an imported model embeds (`assets/soldier.fbx#tex/Skin`): that is what instancing an
`.fbx` with embedded textures puts on its material, resolved through the import cache
whenever the texture is read, so a saved scene never names a cached file (#1130).
`<name>` is the FBX texture element's own name, or, for a `.blend`, the name of an
image packed into it (`assets/crate.blend#tex/Planks`). A texture with no name, one whose
name starts with `#`, and every texture after the first to share a name is named by
its position among the model's textures instead: `assets/soldier.fbx#tex/#2`. That
form shifts when a texture is inserted before it in the FBX, so give textures
distinct names. A texture that is a file on disk beside the model is not embedded and
stays its plain path (`models/skin.png`), in a `.blend` too: only an image the
conversion wrote into the import cache is a reference (#1072).

| Function | Signature |
|---|---|
| `Material.SetMetallic` | `(id, value)` — clamped to `[0,1]` (NaN or `math.huge` reads as 0) |
| `Material.SetRoughness` | `(id, value)` — clamped to `[0,1]` (NaN or `math.huge` reads as 0) |
| `Material.SetColor` | `(id, {r, g, b})` — the base-colour (albedo tint) factor, Unity's `material.color`; each channel clamped to `[0,1]`, and a colour with a NaN or infinite channel is ignored |
| `Material.SetEmissive` | `(id, {r, g, b})` — the flat emissive factor (self-illumination; values >1.0 bloom on the HDR target; each channel floored at 0, and a colour with a NaN or infinite channel is ignored) |
| `Material.SetMetallicMap` | `(id, path)` — sampled by the renderer; the map's blue channel scales the metallic value (glTF metallic-roughness convention) |
| `Material.SetRoughnessMap` | `(id, path)` — sampled by the renderer; the map's green channel scales the roughness value |
| `Material.SetNormalMap` | `(id, path)` — sampled by the renderer; perturbs the shading normal in tangent space (per-vertex tangents + TBN) |
| `Material.SetEmissiveMap` | `(id, path)` — sampled by the renderer; modulates the emissive factor (`factor × map.rgb`) |
| `Material.SetTexture` | `(id, path)` — the albedo map (sampled today) |
| `Material.SetRenderMode` | `(id, mode)` — `"Opaque"` (default), `"Cutout"`, or `"Transparent"` (case-insensitive; an unknown name falls back to Opaque) |
| `Material.SetRenderFace` / `GetRenderFace` | `(id, face)` [`render_face`] — URP Lit's *Render Face*, which faces of the mesh draw: `"Front"` (default, for opaque, cutout and transparent alike: back faces are culled, so a closed mesh never shows its inside), `"Back"` (only the faces turned away: the inside of a dome or a sky sphere) or `"Both"` (foliage cards, a glass pane, water seen from below). A face seen from behind is shaded with its normal flipped toward the viewer, so a `Both` plane is lit from whichever side it is seen. Every pass honours it: the opaque and transparent passes, the depth prepass and postpass, the shadow casters (a `Both` material casts from both faces, as `ShadowCasting` `"TwoSided"` does) and the selection outline. A mesh with a mirroring scale (one negative axis) keeps the same faces. Case-insensitive; an unknown name is an error naming the three. A `.gltf`/`.glb` material marked `doubleSided` imports as `"Both"` (as Unity's glTFast reads it); a `.blend` imports `"Front"`, whatever its Backface Culling, as Unity's `.blend` import does (Blender's glTF exporter marks every material whose Backface Culling is off, Blender's default, `doubleSided`), and an `.fbx` imports `"Front"`. Saved with the material; the Inspector's Material card shows it under the render mode |
| `Material.SetAlpha` | `(id, a)` — base-color alpha in `[0,1]`; the blend factor for a `Transparent` material (ignored by Opaque/Cutout) |
| `Material.SetAlphaCutoff` | `(id, c)` — alpha-test threshold in `[0,1]` for `Cutout`: fragments below it are discarded (default 0.5) |
| `Material.SetShader` | `(id, name)` — render with the authored surface shader `name` (see *Surface shaders* below); `""` clears it back to the standard shader |
| `Material.SetShaderParam` | `(id, name, value)` — override a runtime param of the material's surface shader (`"hit_flash.amount"`, see *Runtime shader params* below) **for entity `id` alone**; its material stays shared and unchanged. `value` is a number or an array of numbers; errors on a param the shader does not expose at runtime |
| `Material.GetShaderParam` | `(id, name)` → the value entity `id` draws with (a number, or an array for a vector param): its override, else its material's value, else the shader's baked default |
| `Material.ClearShaderParam` | `(id [, name])` — drop entity `id`'s override of `name`, or all of its overrides; it draws with its material's values again |
| `Material.SetAssetShaderParam` | `(asset, name, value)` — set a runtime param on the **material asset** named `asset` (Unity's `sharedMaterial`): every entity using it sees it, unless it overrides the param; saves with the scene. Errors on an absent asset |
| `Material.GetAssetShaderParam` | `(asset, name)` → the asset's value: the one set, else the shader's baked default |
| `Material.SetShaderTexture` | `(id, slot, path)` — point an extra texture slot of the material's surface shader (`"mask"`, see *Shader textures* below) at a texture path (a PNG or an `rt:<name>` render texture); `nil` or `""` clears it back to white; errors on an unknown slot |

### Map coordinates: tiling, offset and world mapping

Where a material reads its maps (#1573) follows Unity: URP/HDRP Lit's **Tiling** and
**Offset**, and HDRP Lit's **Base UV Mapping**. They apply to every main map at once
(base colour, metallic, roughness, normal, emissive); there is no per-map tiling.
Each is a field of the material, with a `Set` and a `Get`, saved with it (written
to the scene file even at its default), taken by `DefineAsset` by the key
in brackets, and shown in the Inspector's Material card under the maps.

| Function | Signature |
|---|---|
| `Material.SetTiling` / `GetTiling` | `(id, {x, y})` [`tiling`] — Unity's *Tiling*, `mainTextureScale`: how many times the maps repeat across UV0, or across `world_size` metres under a world mapping. Default `{1, 1}`; each lane clamped to `[-1000, 1000]`. Takes `{4, 2}` or `{x = 4, y = 2}`, returns `{x, y}` by position |
| `Material.SetOffset` / `GetOffset` | `(id, {x, y})` [`offset`] — Unity's *Offset*, `mainTextureOffset`, in texture repeats, added after the tiling. Default `{0, 0}` |
| `Material.SetUvMapping` / `GetUvMapping` | `(id, mode)` [`uv_mapping`] — HDRP's *Base UV Mapping*: `"UV0"` (default: the mesh's UVs), `"Planar"` (projected straight down world Y: floors and terrain tile over world X and Z, walls stretch) or `"Triplanar"` (projected along each world axis and blended by the surface normal: every face of any mesh shows the texture at the same size in metres, however the entity is scaled) |
| `Material.SetWorldSize` / `GetWorldSize` | `(id, metres)` [`world_size`] — how many metres one repeat of the texture covers under `Planar` and `Triplanar`, in `[0.01, 100]` (default 1; HDRP's *World Scale* is its inverse). Ignored under `UV0` |
| `Material.SetBlendSharpness` / `GetBlendSharpness` | `(id, power)` [`blend_sharpness`] — how sharply `Triplanar` turns from one projection to the next across a rounded face, in `[1, 64]` (default 4): the normal's components are raised to this power before they weight the three projections, so 1 blends widely and 64 is a hard seam |

Under `Planar` and `Triplanar` the normal map has no mesh tangents to lean on: each
projection's sample is reoriented onto its world axis (the *whiteout* blend), so a
brick wall's bumps face out of every side of a box. The projections are in world
space, so the texture stays put as an entity moves through it: use world mapping
for static level geometry (a greybox of scaled primitives sharing one material), and
UV0 for anything that moves. A surface shader's UV blocks (`uv_scroll`, see *Surface
shaders* below) move UV0, so they apply under `UV0` only. A `Cutout` material's
shadow is clipped where its surface is, through the same Tiling, Offset and mapping.

```lua with=id
-- Two boxes of different scale, one material: the same 0.5 m tiles on every face.
Material.SetUvMapping(id, "Triplanar")
Material.SetWorldSize(id, 0.5)
Material.SetTiling(id, {1, 1})
```

### Transparent surfaces

A `Transparent` material does more than fade by `alpha` (#1523). Each knob below is a
field of the material (it saves with it, `DefineAsset` takes it by the key in
brackets, and the Inspector's Material card shows it), with a `Set` and a `Get`.
`Get` returns `nil` for an entity without a material; `Set` creates one, as every
setter does, and raises an error naming what the field takes for a value it can't
(an unknown name, a number for a boolean). Names are read in any case. The knobs
follow Unity: URP Lit's blending options, and HDRP Lit's refraction, absorption and
thin translucency, leaving out what HDRP spends most on (diffusion profiles,
sphere and ray-traced refraction, order-independent transparency).

| Function | Signature |
|---|---|
| `Material.SetBlendMode` / `GetBlendMode` | `(id, mode)` [`blend_mode`] — URP's *Blending Mode*: `"Alpha"` (default; `alpha` of the surface over the rest), `"Premultiply"` (the colour is already premultiplied: it is added whole, covering `alpha` of what is behind), `"Additive"` (added onto what is behind, scaled by `alpha`), `"Multiply"` (multiplies what is behind; `alpha` fades the tint toward white) |
| `Material.SetPreserveSpecular` / `GetPreserveSpecular` | `(id, on)` [`preserve_specular`] — URP's *Preserve Specular Lighting*, on by default: only the diffuse is faded by `alpha`, so reflections and highlights stay whole on clear glass, and the opacity rises toward grazing angles with the Fresnel reflection (URP's `alpha * (1 - reflectivity) + reflectivity`, at the view angle), so water seen flat reflects more and shows less of the floor. Ignored under `Premultiply` and `Multiply`, as URP ignores it |
| `Material.SetRefractionModel` / `GetRefractionModel` | `(id, model)` [`refraction_model`] — HDRP's *Refraction Model*: `"None"` (default), `"Planar"` (the surface is a slab `thickness` thick: water, a thick pane; HDRP's *Planar*, its code's `RefractionModelBox`) or `"Thin"` (HDRP's thin shell, a fixed 5 mm slab, so a window or a bubble barely bends). The view ray bends by the shading normal, its normal map included, so ripples bend the floor; where the bent ray would land on something in front of the surface it reads straight through, as HDRP does |
| `Material.SetIndexOfRefraction` / `GetIndexOfRefraction` | `(id, ior)` [`ior`] — HDRP's *Index of Refraction* in `[1, 2.5]`: water 1.33, glass 1.5 (the default) |
| `Material.SetThickness` / `GetThickness` | `(id, metres)` [`thickness`] — how thick the surface is, in `[0, 10]` (default 1), as HDRP's *Thickness*: a `Planar` refraction's slab depth (a `Thin` one is always 5 mm), and the depth an absorbing surface absorbs over where only the sky is behind it, whatever its refraction model |
| `Material.SetTransmittanceColor` / `GetTransmittanceColor` | `(id, {r, g, b})` [`transmittance_color`] — HDRP's *Transmittance Color*: the colour light keeps after `absorption_distance` metres through the surface (Beer-Lambert), each channel in `[0, 1]`. White, the default, absorbs nothing; any other colour tints and darkens what is behind by how far it is behind the surface, so shallow water stays clear and the deep end takes the colour |
| `Material.SetAbsorptionDistance` / `GetAbsorptionDistance` | `(id, metres)` [`absorption_distance`] — HDRP's *Absorption Distance*, where light has kept exactly `transmittance_color`, in `[0.01, 100]` (default 1) |
| `Material.SetDepthPostpass` / `GetDepthPostpass` | `(id, on)` [`depth_postpass`] — HDRP's *Transparent Depth Postpass*, off by default: after the translucents drew, the surface writes its depth, so depth of field focuses on it and motion blur moves it rather than what is behind it, and soft particles and world canvases meet it. Fog needs none: each surface is fogged where it is drawn. SSR reflections on it are `receive_ssr`'s (below), which needs no postpass |
| `Material.SetReceiveSsr` / `GetReceiveSsr` | `(id, on)` [`receive_ssr`] — HDRP's *Receive SSR Transparent*, off by default: where the camera runs screen-space reflections (`Graphics`' SSR, on the base camera), a smooth `Transparent` surface traces its own reflected ray against the opaque scene and puts what it hits in place of its sky or reflection-probe reflection, faded by the same smoothness and screen-edge rules as an opaque surface; a miss keeps the probe. So water shows the player, a ball or an enemy moving above it. It reflects only opaque things (other translucents are not in the opaque copy), and an opaque material always takes SSR, so the knob is shown for `Transparent` only. Cost: one march per pixel the surface covers, at SSR's quality; with SSR off, or no such surface on screen, nothing. A surface too rough to ever reflect (roughness 0.2 or more, with no roughness map) traces nothing and asks for no opaque copy |
| `Material.SetTransmission` / `GetTransmission` | `(id, amount)` [`transmission`] — HDRP's thin *Translucent* type, in `[0, 1]` (default 0): how much of a light behind the surface comes through as diffuse light (a leaf, a lampshade, a cloth banner lit from behind). Any material, opaque or transparent; the surface is shaded with its normal toward the viewer, as HDRP flips a double-sided translucent's |
| `Material.SetTransmissionColor` / `GetTransmissionColor` | `(id, {r, g, b})` [`transmission_color`] — HDRP's *Transmission Tint*, the colour that light takes on its way through (default white); each channel in `[0, 1]` |
| `Material.SetReceiveShadows` / `GetReceiveShadows` | `(id, on)` [`receive_shadows`] — URP's *Receive Shadows*, on by default: off, neither the sun's cascades nor a point or spot light's shadow lands on the surface (a wall's shadow stays off the water). Any material, opaque too; the surface still casts its own |

Refraction and absorption read the **opaque copy** (Unity's *Opaque Texture* and
*Depth Texture*): the camera's colour and depth after its opaque pass and
screen-space reflections, made only on a frame that shows a surface that refracts
or absorbs (or traces SSR with `receive_ssr`), and counted in
`Debug.Stats().opaque_copies`. So what a refracting
surface shows behind it is the opaque scene (other translucents behind it are not
in the copy, as in Unity), and only under `Alpha` or `Premultiply`: an `Additive` or
`Multiply` surface adds to or tints what is behind it, and has nothing to bend.

### Material slots

An entity's `MaterialComponent` holds a **list** of library keys, one per material
run of its mesh, in the mesh's order (Unity's `Renderer.sharedMaterials`, #1285).
Importing a model splits each mesh into runs at every material boundary — a glTF
mesh's primitives, an FBX mesh's per-polygon materials, an `.obj` object's `usemtl`
groups — while the mesh stays one addressable sub-object (`model.fbx::Body`).
Instancing it fills one slot per run with that run's imported material (the engine
default for a run that names none). A primitive, or any one-material mesh, has one
slot. As in Unity, the list and the runs need not match:

- **fewer materials than runs:** a run past the end of the list is not drawn;
- **more materials than runs:** each extra material draws the last run again, over
  it (a second pass, as Unity does).

| Function | Signature | Returns |
|---|---|---|
| `Material.Get` | `(id)` | the first slot's library key (Unity's `sharedMaterial`), or `nil` if the entity has no Material |
| `Material.Set` | `(id, key)` | — points the first slot at library material `key`, attaching a Material if the entity has none |
| `Material.GetAll` | `(id)` | an array of the library key of every slot, in run order (empty if the entity has no Material) |
| `Material.SetAt` | `(id, index, key)` | — points slot `index` (0-based, Unity's `sharedMaterials[i]`) at library material `key`; the other slots are untouched |

`index` is 0-based, like every index argument in rusty's API (`Probe`,
`Reflection`, `Layers`, pads). `GetAll` returns a Lua array, which starts at 1, so a
loop over it passes `i - 1`:

```lua with=e:Material
for i, key in ipairs(Material.GetAll(e)) do
    Material.SetAt(e, i - 1, key)
end
```

`Set` and `SetAt` raise an error, and change nothing, when `index` is past the
entity's slots (one per run of its mesh, or its list's length when that is longer)
or the library has no material `key`. Setting a slot past the end of the list grows
it, every new slot up to `index` taking `key`. The slots save with the scene.

```lua with=e:Material
Material.DefineAsset("chrome", { metallic = 1.0, roughness = 0.1 })
Material.Set(e, "chrome")
print(Material.Get(e), #Material.GetAll(e))   -- chrome  1
```

### Standalone material assets

The setters above mutate the material an *entity* references. To author a reusable
material as a **named asset** from scratch — decoupled from any entity, the way an
imported glTF material is — define it directly into the library, then point an
entity's slot at the name (`Material.Set` / `Material.SetAt`). The asset round-trips through `SceneData`
like any library material (no engine special-casing).

| Function | Signature | Returns |
|---|---|---|
| `Material.DefineAsset` | `(name, recipe)` | — (inserts/overwrites the library asset under `name`) |
| `Material.GetAsset` | `(name)` | the asset's canonical JSON string, or `nil` if absent |
| `Material.Rebake` | `(name [, resolution])` | — (re-bakes the asset's `maps` recipe; errors if it has none) |
| `Material.HasAsset` | `(name)` | `bool` |
| `Material.DeleteAsset` | `(name [, { force = true }])` | an array naming each user it moved to the default material (empty when none); see *Deleting a material asset* below |

The **recipe** is a table (or that table's JSON string — both decode alike) whose shape mirrors the `MaterialAsset` document one-to-one
(every field optional — omit one to take its default; enum values are read in any case), bounded to exactly what the
renderer samples:

```lua with=id
Material.DefineAsset("brick", {
  base_color   = {0.5, 0.25, 0.125},   -- albedo tint (rgb)
  base_color_map = "brick_albedo.png", -- albedo map
  metallic     = 0.0,
  roughness    = 0.8,
  metallic_map = "brick_mr.png",       -- packed metallic→B
  roughness_map = "brick_mr.png",      -- packed roughness→G (same PNG)
  normal_map   = "brick_n.png",        -- tangent-space normal
  emissive     = {0.0, 0.0, 0.0},      -- emissive factor (rgb)
  emissive_map = "brick_e.png",
  render_mode  = "Cutout",             -- "Opaque" (default) / "Cutout" / "Transparent"
  render_face  = "Both",               -- "Front" (default) / "Back" / "Both"
  alpha        = 1.0,                  -- base-color alpha in [0,1]
  alpha_cutoff = 0.4,                  -- alpha-test threshold in [0,1]
  shader       = "brick_toon",         -- authored surface shader ("" / omitted = standard)
})

-- An entity uses the asset by referencing its name:
Scene.AddComponent(id, "Material")   -- attaches a MaterialComponent
-- (point the reference at "brick" — e.g. via the inspector, or load a scene whose
--  entity already references it). Saving the scene persists both the asset and the ref.
```

The map slots point at any PNG — typically one baked by `Texture.Bake` (e.g.
`Material.DefineAsset("brick", { metallic_map = Texture.Bake(recipe,
"out/brick_mr.png", "metallic_roughness"), ... })`), but any PNG works. As in glTF
2.0, `base_color_map` and `emissive_map` are colour and decode as **sRGB**; the
metallic, roughness and normal maps and every `shader_textures` slot hold data and are
sampled **raw** (#647) — so bake those with their linear `Texture.Bake` slot. One PNG
named in both roles is uploaded once and read each way. The factors +
maps decode through the asset's serde derive, while the validated fields are applied
through the *same* shared ops the per-entity setters use: `alpha`/`alpha_cutoff` clamp
to `[0,1]`, and an unknown `render_mode` string degrades to `Opaque` — so validation is
single-sourced, not re-implemented. `Material.DefineAsset` **overwrites** any existing
asset of that name.

#### Deleting a material asset

`Material.DeleteAsset(name)` takes a material out of the library, as deleting a
material asset in Unity does (#1576). Its **users** are what names it by its key:
each material slot of an entity, an emitter's mesh particles (`render.material`)
and a live decal (`Decals.Spawn`'s `material`). A material still in use is
**refused**: the error names every user (`entity 3 "Floor" slot 0`, with
`(instance of prefabs/crate.prefab)` for a prefab instance's entity), and nothing
changes. With `{ force = true }` each user falls back to the engine's default
material and the console warns which: a slot to its entity's own default material
(`entity_<id>_material`, created as the default white material when the library has
none, the one *Add Component* attaches), mesh particles and a decal to no material
(the engine default, and the decal's own texture). An absent `name` is an error. The
deletion saves with the scene. A `.prefab` file is never a user: it carries its own
copy of every material it names, which instancing it merges back in. The editor's
Material card has the same **Delete** (a confirm lists the users) and one *Undo*
that restores the asset and every slot it moved.

```lua with=id
Material.DefineAsset("checker", { base_color = {0.5, 0.5, 0.5} })
Material.Set(id, "checker")
-- Material.DeleteAsset("checker") would refuse: entity `id` uses it.
local moved = Material.DeleteAsset("checker", { force = true })
-- moved[1] names entity `id`'s slot 0, now on its own default material.
```

#### One call: the maps recipe inside the material — `maps`

Instead of baking each map with `Texture.Bake`/`Texture.BakeSet` and passing the
paths, give the material its **multi-output texture recipe** as `maps` (the same
document `Texture.BakeSet` takes, `outputs` and all). `DefineAsset` bakes it once to
`assets/textures/<name>_<slot>.png` and fills the matching map keys (the
`Texture.BakeSet` result table in `Texture.md` lists which slot fills which key —
`metallic_roughness` fills both `metallic_map` and `roughness_map`):

```lua no-run
Material.DefineAsset("rusty_panel", {
  roughness = 1.0, metallic = 1.0,            -- factors scale the maps, as always
  normal_map = "hand_made_n.png",             -- an explicit map key wins over the bake
  shader = "enemy_toon",                      -- every other key works as above
  maps = {
    resolution = 256, seed = 3,
    nodes = { ... },
    outputs = { base_color = "albedo", normal = "n", metallic_roughness = "mr" },
  },
})
Material.Rebake("rusty_panel", 2048)          -- iterate at 256, ship at 2048
```

- The **recipe is stored on the asset** (`maps` in its JSON and the scene file), so
  the material describes itself: `GetAsset` returns it, and that JSON redefines the
  same material. The baked PNGs are a cache the recipe regenerates.
- `Material.Rebake(name [, resolution])` re-bakes from the stored recipe to the same
  paths; a `resolution` given is kept in the recipe for the next rebake. A `maps`
  recipe with a `size` (a non-square bake) keeps its aspect, its longer side
  becoming `resolution` (`Texture`, *Non-square bakes*). It is an
  error when `name` is absent or has no `maps`. The next frame draws the new maps
  (#689).
- A map key the material **already holds** is never overwritten by a bake — set by
  hand in the recipe, it wins; one the bake filled points at the same file next time.
- Everything is checked before a file is written or the library changes: a bad `maps`
  recipe (an unknown slot, node id or key), or a `name` holding `/` or `\`, is an
  error and `DefineAsset` defines nothing.

An **unknown recipe key** (`metalic_map`) is an error naming it and listing the valid
keys, like every authoring recipe (`Texture`, `Shader`, `Sound`). The **deliberate
exception** is `render_mode`'s *value*: an unrecognized mode degrades to `Opaque`
rather than erroring, matching `Material.SetRenderMode` — opaque is the safe, always
visible fallback.

> **Rendering modes (transparency).** A material's `render_mode` controls how its
> surface composites (#242), mirroring Unity's rendering modes:
> - **Opaque** *(default)* — fully solid, the fast path. `alpha`/`alpha_cutoff` are
>   inert.
> - **Cutout** — alpha-tested hard edges (foliage, chain-link, grates): a fragment
>   whose sampled alpha is below `alpha_cutoff` is discarded, the rest is opaque. Still
>   writes depth and needs no sorting. Its shadow is cut the same way (#648): a leaf
>   card shadows its leaf, not its quad.
> - **Transparent** — alpha-blended (glass, holograms, fades): the surface blends over
>   what is behind it using `alpha` (× the texture's alpha) as the opacity. Drawn in a
>   separate pass after the opaque geometry, sorted back-to-front per object, with depth
>   testing on but depth writes off — so overlapping translucent surfaces all blend
>   correctly regardless of draw order. (Per-object sort only; intersecting translucent
>   surfaces within one mesh are a known limitation, as in Unity's default.)
>
> The current `render_mode`/`alpha`/`alpha_cutoff` are readable back via the scene
> snapshot's per-entity `material` block.

> **All glTF-PBR maps are sampled.** The albedo, metallic, roughness, normal, and
> emissive maps (and the flat emissive factor) all reach the forward shader. Normal
> mapping uses a per-vertex `tangent` attribute — read from the glTF `TANGENT` accessor
> when present, else generated from positions + UVs at import — so it works on imported
> meshes and the engine's procedural primitives alike (#207).

> **Imported materials.** Instantiating a glTF model populates the material library
> from the file's authored metallic-roughness materials and points each sub-object's
> entity at the matching library entries via its `MaterialComponent`, one slot per
> material run of its mesh (*Material slots*). The library key
> is deterministic — `"<path>::<material-name>"` (or `"<path>::material_<index>"`
> when the material is unnamed) — so sub-objects that share one glTF material share a
> single library entry. glTF packs metallic + roughness in one texture; on import it
> is mapped to *both* the engine's `metallic_map` and `roughness_map` (the shader
> reads the blue/green channel from each). External texture URIs resolve to paths
> relative to the glTF file; embedded images are not yet extracted (a follow-up). OBJ
> imports only `Kd` (base color) and `map_Kd` (albedo) — it is static-mesh only.

> **Starter materials.** The engine ships a small ready-to-go set so a new scene has
> sane PBR values to grab instead of starting from raw factors. They live as an
> ordinary glTF asset at `assets/materials/starter.gltf` — **no special-casing**: it
> is discovered by `Assets.Manifest()` and imported through the same path as any user
> model. The four materials are `Matte` (rough dielectric), `Metal` (polished
> conductor), `Plastic` (smooth coloured dielectric), and `Emissive` (a glowing
> dielectric). Instantiating the file populates the library with keys
> `assets/materials/starter.gltf::Matte` (and so on for `Metal` / `Plastic` /
> `Emissive`); point any entity's slot at one of those keys (`Material.Set`) to reuse it,
> or copy its factors as the starting point for your own. No external textures — the
> factors stand alone, so the set stays tiny and license-clean.

### Decals: `receive_decals` and `decal`

Two recipe keys concern surface decals (#638, see `Decals.md`):

- **`receive_decals`** (default `true`) — whether surfaces drawn with this material
  take world decals: HDRP's *Receive Decals*. Set it `false` for characters and
  props that move through bullet holes. Transparent materials never receive.
- **`decal`** — what this material changes when a decal stamps it
  (`Decals.Spawn(…, { material = name })`): per-channel blend weights `albedo`,
  `normal`, `metallic`, `roughness` (default `1` each, clamped to `[0, 1]`), the
  `occlusion` it adds (default `1`, none) and `angle_fade` in degrees (default `60`).
  An unknown key inside it is an error.

```lua
Material.DefineAsset("enemy_skin", { base_color = {0.8, 0.6, 0.5}, receive_decals = false })
Material.DefineAsset("wet", { roughness = 0.05, decal = { albedo = 0, normal = 0, metallic = 0 } })
```

Both are written to the scene file only when they differ from their defaults.

### Surface shaders

A material **names the shader it renders with** — Unity's Material → Shader. The
name is a surface module baked by `Shader.Bake` (see `Shader.md`): `"enemy_toon"`
resolves to the project's `assets/shaders/enemy_toon.wgsl` (where bakes land), else
the engine's `engine/shaders/enemy_toon.wgsl`. No name (the default) is the standard forward
shader.

```lua no-run
Shader.Bake({ pass = "surface", name = "enemy_toon",
              blocks = { { id = "toon_ramp", params = { steps = 3 } } } })
Material.SetShader(enemy, "enemy_toon")   -- or `shader = "enemy_toon"` in a recipe
```

- Every entity sharing the material draws with the shader, opaque and transparent
  alike. Its shadow and SSAO depth match the standard shader's, except where a
  block cuts fragments (`dissolve`): those are cut from the shadow and the SSAO
  depth too (#648). A block that moves the UVs (`uv_scroll`) moves them there too,
  so a Cutout material clips its shadow and depth where its colour shows (#1499). A
  block that moves the vertices (`vertex_waves`) moves them there too, so the shadow
  and depth follow the moved surface (#1524).
- The module is compiled the first frame a material uses it. A name that is not
  there, a module that fails to compile, or a `postfx` module logs one warning and
  **renders with the standard shader** — a bad shader never crashes the game. A
  shader baked *after* the material names it is picked up on the next frame.
- **Re-baking** a shader in a running session rebuilds it: the bake → look →
  iterate loop needs no restart.
- `Debug.Snapshot`'s `material` block reports it as `shader` (`null` = standard),
  the runtime param values set on the material as `shader_params`, and its shader textures as
  `shader_textures`.

### Runtime shader params

Some surface block params are **runtime** (see `Shader.md`): instead of being baked
into the shader, they are read at draw time, so a script drives them every frame —
Unity's `material.SetFloat`. A hit flash that fades, a rim that glows with a
shield's charge:

```lua no-run
Shader.Bake({ pass = "surface", name = "enemy_hit",
              blocks = { { id = "toon_ramp" },
                         { id = "hit_flash", params = { color = {1, 0.2, 0.1} } } } })
Material.DefineAsset("grunt_mat", { shader = "enemy_hit" })   -- shared by every grunt

-- on hit: only this grunt flashes, the others sharing grunt_mat do not
Material.SetShaderParam(enemy, "hit_flash.amount", 1)
-- and let it fade out over a tenth of a second (a tween of this entity's override):
Tween.To(enemy, "Material.hit_flash.amount", 0, 0.1, { from = 1 })
-- or, once the flash is over, hand it back to the material:
Material.ClearShaderParam(enemy, "hit_flash.amount")

-- the shared value, for every grunt at once (a squad-wide shield glow):
Material.SetAssetShaderParam("grunt_mat", "fresnel_rim.strength", 2)
```

- **Names** are `"<block>.<param>"`, or `"<block>.<index>.<param>"` (the block's
  position in the recipe's `blocks`), which is required when the recipe uses that
  block more than once. Both forms set the same value.
- **Strict.** A param the shader bakes (`toon_ramp.steps`), a typo, or the wrong
  number of values is an error naming it and listing the runtime params the shader
  has. A single number fills every lane of a vector param (`color = 0.5`). The
  material must name a baked surface shader first.
- **Per entity, or per material** — Unity's `renderer.material` /
  `MaterialPropertyBlock` beside `sharedMaterial`. `SetShaderParam(id, …)` writes an
  **override** for that entity only: it wins over the material's value for that
  entity, and the material stays shared, so ten grunts on one `grunt_mat` flash one
  at a time. Overrides are **runtime-only**: never saved, gone when the entity is
  destroyed, when a scene loads, and on Stop. `SetAssetShaderParam(asset, …)` writes
  the **material's** value (`shader_params` on the asset), which saves with the scene
  and which every entity without an override draws with; play mode changes the play
  copy, so it never leaks into the edit scene either.
- **Tweening.** `Tween.To(id, "Material.<name>", …)` eases entity `id`'s
  override over time (#661) — a hit flash fading, a dissolve on death — through the
  same check as `SetShaderParam`; see [`Tween.md`](Tween.md). The shared material
  value is not tweenable.
- **Batching.** Entities without overrides keep drawing together as one instanced
  draw. Each entity with an override is a draw of its own (twenty enemies on one
  material with five flashing: six forward draws instead of one), so clear an
  override once its effect is over.
- **Cheap.** A change is a small buffer write the next frame draws with. Nothing is
  re-baked or recompiled.
- A param you never set draws with its baked default (the recipe's value, else the
  block's). The inspector's Material card lists the runtime params under *Shader*.

### Shader textures

A surface shader's blocks may sample an **extra texture** the material names — a
noise for `dissolve`, a grime map for `detail_overlay` (see `Shader.md`, *Extra
textures*): Unity's `material.SetTexture("_Mask", …)`.

```lua with=enemy
Material.SetShaderTexture(enemy, "mask", "assets/textures/noise.png")
-- or in a recipe:
Material.DefineAsset("enemy_die", { shader = "enemy_die",
                                    shader_textures = { mask = "noise.png" } })
```

- **Slots.** v1 has one, `mask`. An unknown slot (`"detail"`) is an error naming
  it, from `SetShaderTexture` and `DefineAsset` alike (a bad recipe defines
  nothing).
- **Never a crash.** A slot left unset, or a file that is missing, samples **1×1
  white**. The path is not checked when set, so a texture made later binds when it
  appears; an `rt:<name>` render texture works like any other map.
- **Per material**, saved with the scene as `shader_textures`; `Material.GetAsset`
  returns it. The inspector's Material card shows one path field per slot once
  the material names a shader.
