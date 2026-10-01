## `Material`

A *material* is a reusable **asset** — glTF 2.0 metallic-roughness data — stored
once in the scene's per-World **material library**; an entity carries only a thin
**reference** to one by name (its `MaterialComponent`). Many entities can share a
single material, so editing it once updates all of them. These functions resolve
the entity's referenced material in the library and mutate that shared asset;
calling one on an entity that has no material yet **creates** a default library
material and attaches the reference. `SetTexture` sets the albedo (`base_color`)
map; an empty path clears it.

| Function | Signature |
|---|---|
| `Material.SetMetallic` | `(id, value)` |
| `Material.SetRoughness` | `(id, value)` |
| `Material.SetEmissive` | `(id, {r, g, b})` — the flat emissive factor (self-illumination; values >1.0 bloom on the HDR target) |
| `Material.SetMetallicMap` | `(id, path)` — sampled by the renderer; the map's blue channel scales the metallic value (glTF metallic-roughness convention) |
| `Material.SetRoughnessMap` | `(id, path)` — sampled by the renderer; the map's green channel scales the roughness value |
| `Material.SetNormalMap` | `(id, path)` — sampled by the renderer; perturbs the shading normal in tangent space (per-vertex tangents + TBN) |
| `Material.SetEmissiveMap` | `(id, path)` — sampled by the renderer; modulates the emissive factor (`factor × map.rgb`) |
| `Material.SetTexture` | `(id, path)` — the albedo map (sampled today) |
| `Material.SetRenderMode` | `(id, mode)` — `"Opaque"` (default), `"Cutout"`, or `"Transparent"` (case-insensitive; an unknown name falls back to Opaque) |
| `Material.SetAlpha` | `(id, a)` — base-color alpha in `[0,1]`; the blend factor for a `Transparent` material (ignored by Opaque/Cutout) |
| `Material.SetAlphaCutoff` | `(id, c)` — alpha-test threshold in `[0,1]` for `Cutout`: fragments below it are discarded (default 0.5) |
| `Material.SetShader` | `(id, name)` — render with the authored surface shader `name` (see *Surface shaders* below); `""` clears it back to the standard shader |
| `Material.SetShaderParam` | `(id, name, value)` — set a runtime param of the material's surface shader (`"hit_flash.amount"`, see *Runtime shader params* below); `value` is a number or an array of numbers; errors on a param the shader does not expose at runtime |
| `Material.GetShaderParam` | `(id, name)` → the param's current value (a number, or an array for a vector param): the one set, else the shader's baked default |

### Standalone material assets

The setters above mutate the material an *entity* references. To author a reusable
material as a **named asset** from scratch — decoupled from any entity, the way an
imported glTF material is — define it directly into the library, then point any
entity's `MaterialComponent` at the name. The asset round-trips through `SceneData`
like any library material (no engine special-casing).

| Function | Signature | Returns |
|---|---|---|
| `Material.DefineAsset` | `(name, recipe)` | — (inserts/overwrites the library asset under `name`) |
| `Material.GetAsset` | `(name)` | the asset's canonical JSON string, or `nil` if absent |
| `Material.HasAsset` | `(name)` | `bool` |

The **recipe** is a table (or that table's JSON string — both decode alike) whose shape mirrors the `MaterialAsset` document one-to-one
(every field optional — omit one to take its default), bounded to exactly what the
renderer samples:

```lua
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
"out/brick_mr.png", "metallic_roughness"), ... })`), but any PNG works. The factors +
maps decode through the asset's serde derive, while the validated fields are applied
through the *same* shared ops the per-entity setters use: `alpha`/`alpha_cutoff` clamp
to `[0,1]`, and an unknown `render_mode` string degrades to `Opaque` — so validation is
single-sourced, not re-implemented. `Material.DefineAsset` **overwrites** any existing
asset of that name.

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
>   writes depth and needs no sorting.
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
> entity at the matching library entry via its `MaterialComponent`. The library key
> is deterministic — `"<path>::<material-name>"` (or `"<path>::material_<index>"`
> when the material is unnamed) — so sub-objects that share one glTF material share a
> single library entry. glTF packs metallic + roughness in one texture; on import it
> is mapped to *both* the engine's `metallic_map` and `roughness_map` (the shader
> reads the blue/green channel from each). External texture URIs resolve to paths
> relative to the glTF file; embedded images are not yet extracted (a follow-up). OBJ
> imports only `Kd` (base color) and `map_Kd` (albedo) — it is static-mesh only.

> **Starter materials.** The engine ships a small ready-to-go set so a new scene has
> sane PBR values to grab instead of starting from raw factors. They live as an
> ordinary glTF asset at `project/materials/starter.gltf` — **no special-casing**: it
> is discovered by `Assets.Manifest()` and imported through the same path as any user
> model. The four materials are `Matte` (rough dielectric), `Metal` (polished
> conductor), `Plastic` (smooth coloured dielectric), and `Emissive` (a glowing
> dielectric). Instantiating the file populates the library with keys
> `project/materials/starter.gltf::Matte` (and so on for `Metal` / `Plastic` /
> `Emissive`); point any entity's `MaterialComponent` at one of those keys to reuse it,
> or copy its factors as the starting point for your own. No external textures — the
> factors stand alone, so the set stays tiny and license-clean.

### Surface shaders

A material **names the shader it renders with** — Unity's Material → Shader. The
name is a surface module baked by `Shader.Bake` (see `Shader.md`): `"enemy_toon"`
resolves to `project/assets/shaders/enemy_toon.wgsl` (where bakes land), else
`assets/shaders/enemy_toon.wgsl`. No name (the default) is the standard forward
shader.

```lua
Shader.Bake({ pass = "surface", name = "enemy_toon",
              blocks = { { id = "toon_ramp", params = { steps = 3 } } } })
Material.SetShader(enemy, "enemy_toon")   -- or `shader = "enemy_toon"` in a recipe
```

- Every entity sharing the material draws with the shader, opaque and transparent
  alike; the shadow and depth passes are unchanged (a surface shader only restyles
  colour).
- The module is compiled the first frame a material uses it. A name that is not
  there, a module that fails to compile, or a `postfx` module logs one warning and
  **renders with the standard shader** — a bad shader never crashes the game. A
  shader baked *after* the material names it is picked up on the next frame.
- **Re-baking** a shader in a running session rebuilds it: the bake → look →
  iterate loop needs no restart.
- `Debug.Snapshot`'s `material` block reports it as `shader` (`null` = standard),
  and the runtime param values set on it as `shader_params`.

### Runtime shader params

Some surface block params are **runtime** (see `Shader.md`): instead of being baked
into the shader, they are read from the material, so a script drives them every
frame — Unity's `material.SetFloat`. A hit flash that fades, a rim that glows with a
shield's charge:

```lua
Shader.Bake({ pass = "surface", name = "enemy_hit",
              blocks = { { id = "toon_ramp" },
                         { id = "hit_flash", params = { color = {1, 0.2, 0.1} } } } })
Material.SetShader(enemy, "enemy_hit")

-- on hit:
Material.SetShaderParam(enemy, "hit_flash.amount", 1)
-- each frame:
local a = Material.GetShaderParam(enemy, "hit_flash.amount")
Material.SetShaderParam(enemy, "hit_flash.amount", math.max(a - Time.deltaTime() * 10, 0))
```

- **Names** are `"<block>.<param>"`, or `"<block>.<index>.<param>"` (the block's
  position in the recipe's `blocks`), which is required when the recipe uses that
  block more than once. Both forms set the same value.
- **Strict.** A param the shader bakes (`toon_ramp.steps`), a typo, or the wrong
  number of values is an error naming it and listing the runtime params the shader
  has. A single number fills every lane of a vector param (`color = 0.5`). The
  material must name a baked surface shader first.
- **Per material.** The value lives on the material asset (`shader_params`), so it
  saves with the scene and every entity sharing the material sees it. Play mode
  changes the play copy, so a flash in play never leaks into the edit scene. To flash
  one enemy alone, give it its own material.
- **Cheap.** A change is a small buffer write the next frame draws with. Nothing is
  re-baked or recompiled.
- A param you never set draws with its baked default (the recipe's value, else the
  block's). The inspector's Material card lists the runtime params under *Shader*.
