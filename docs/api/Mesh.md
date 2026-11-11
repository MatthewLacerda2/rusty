## `Mesh`

Two things live here: **meshes made from data** in a script (#2059, Unity's `Mesh`
class and `MeshFilter.sharedMesh`), and the per-renderer settings of an entity's
`MeshComponent` (#1714, Unity's *Mesh Renderer*).

| Function | Arguments | Returns |
|---|---|---|
| `Mesh.Create` | `(name, { positions, indices, normals?, uvs? })` | — defines or replaces the scene mesh `name`; a Lua error on bad data |
| `Mesh.GetAsset` | `(name)` | the mesh's data, in the table form `Create` takes; `nil` when there is none |
| `Mesh.GetMaxDrawDistance` | `(id)` | the distance in metres past which a camera stops drawing it; `0` = unlimited |
| `Mesh.GetMinScreenSize` | `(id)` | the screen-relative height (`0..1`) under which a camera stops drawing it; `0` = off |
| `Mesh.GetSharedMesh` | `(id)` | what the entity's Mesh draws: a primitive's name (`"Box"`), a scene mesh's name, or a model reference (`path::sub_object`); `nil` without a Mesh |
| `Mesh.HasAsset` | `(name)` | whether the scene defines a mesh `name` |
| `Mesh.SetMaxDrawDistance` | `(id, metres)` | — negative, NaN or infinite is `0` |
| `Mesh.SetMinScreenSize` | `(id, height)` | — held to `[0, 1]`; NaN or infinite is `0` |
| `Mesh.SetSharedMesh` | `(id, name)` | — the entity draws scene mesh `name` or the primitive `name` (adding a Mesh when it has none); a Lua error for an unknown name |

### Meshes made from data

`Mesh.Create(name, data)` puts a mesh in the open scene's **mesh library**, beside
its material library ([`Material.DefineAsset`](Material.md)); `Mesh.SetSharedMesh`
makes an entity draw it. That is how a game cuts an object in two, deforms a
surface, builds debris or a level piece from code. A drawn scene mesh is a mesh like
any other: lit, taking the entity's [`Material`](Material.md), casting and receiving
shadows ([`Light`](Light.md#cast-shadows)), culled like the rest.

- **The data is flat, 0-based arrays.** `positions` is x,y,z triples, `indices`
  lists triangles (three per face, counter-clockwise seen from the front, numbered
  from 0), `normals` (x,y,z triples) and `uvs` (u,v pairs) are optional and, when
  given, one per vertex. Normals left out are computed, area-weighted (Unity's
  `RecalculateNormals`): a vertex shared by two faces gets a smooth normal, so give
  a hard edge its own vertices. UVs left out are all zero. Tangents are built from
  the UVs (MikkTSpace) as for every mesh; there is no lightmap UV, so a static
  scene mesh reads light probes, not a baked lightmap. Bounds are always the
  vertices'.
- **Refusals are Lua errors**, naming the verb and the reason: a count that is not a
  whole number of triples (or pairs, or triangles), an index that is negative, not
  a whole number or past the last vertex, normals or UVs not one per vertex, a NaN
  or infinite value, and more than **1,048,576 vertices** or **6,291,456 indices**
  (2^20 vertices, two triangles each: past it a runaway loop fails with a message
  instead of allocating gigabytes). A refused call changes nothing. An empty mesh
  is allowed and draws nothing.
- **Names.** A name is not empty, holds no `::` (a model reference's separator)
  and is not a primitive's (`Box`, `Sphere`, `Plane`, `Cylinder`, `Capsule`, the
  lights), since `SetSharedMesh` reads that as the primitive.
- **Replacing is shared.** `Mesh.Create` on an existing name replaces it, and
  every entity drawing it redraws (Unity's shared mesh edited in place). They share
  one GPU upload, made once per change, not per frame or per entity.
- **Saved with the scene.** In edit mode the library is saved in the scene file
  (`meshes`, left out while empty) and an entity saves only the name it draws.
  `Mesh.Create` and `SetSharedMesh` are undone by Undo like any edit. In Play, a
  mesh made or replaced is Play state: Stop puts the edit-mode library back. A
  `DontDestroyOnLoad` entity carries the mesh it draws into a loaded scene, unless
  that scene defines one of the same name, which it then draws.
- **Not in a prefab.** A scene mesh lives in its scene only, so saving or applying
  a prefab whose entities draw one is refused with the entity's name.
- **Colliders.** A [`Mesh` collider](Physics.md) on the entity (convex or not)
  builds from the scene mesh as from any Mesh, and follows a replace. The
  simulation reads only this CPU data, never the GPU.
- `SetSharedMesh` switches an entity between a primitive and a scene mesh; it keeps
  the Mesh's own settings (shadows, culling, Scale In Lightmap). A model's sub-mesh
  is placed with [`Scene.Instantiate`](Scene.md), not here.

```lua
-- A quad on the ground, then a tetrahedron with computed normals.
Mesh.Create("quad", {
  positions = { 0,0,0,  1,0,0,  1,0,-1,  0,0,-1 },
  indices   = { 0,1,2,  0,2,3 },
  uvs       = { 0,0,  1,0,  1,1,  0,1 },
})
local floor = Scene.CreateEntity("Floor")
Mesh.SetSharedMesh(floor, "quad")

Mesh.Create("tetra", {
  positions = { 0,0,0,  1,0,0,  0,0,-1,  0,1,0 },
  indices   = { 0,2,1,  0,1,3,  1,2,3,  2,0,3 },
})
local rock = Scene.CreateEntity("Rock", "Box")
Mesh.SetSharedMesh(rock, "tetra")
print(Mesh.GetSharedMesh(rock), #Mesh.GetAsset("tetra").positions) --> tetra 12
```

### Small-object cull

Today the per-renderer settings are the **small-object cull**: a prop that is
too far away, or too small on screen, is not drawn. A 20 cm can on a shelf across
a 50 m hall covers a few pixels: drawing it costs a draw, its shadow and its
lighting, and shows nothing. Both settings are off by default, so a scene that sets
neither draws exactly what it drew before.

Without a Mesh, getters return `0` and setters do nothing. The same fields are
`max_draw_distance` and `min_screen_size` to `Component.Get` / `Component.Set(id,
"Mesh", { … })` ([`Component`](Component.md)), the Inspector's Mesh card shows them,
and the scene and its prefabs save them when set.

**Max Draw Distance** is Unreal's: measured from the camera to the centre of the
mesh's world bounds. **Min Screen Size** is the measure Unity's `LODGroup` picks
levels by ([`LODGroup`](LODGroup.md)), and so the same value as its *Culled*
share: the mesh's size over the height the view spans at that distance,
`size / (2 · distance · tan(fov / 2))`; an orthographic camera measures
`size / (2 · half-height)` at any distance. The size is fixed per mesh, as a
group's is: the largest extent of its local bounds times its transform's largest
axis scale, so rotating it never changes the verdict (a skinned mesh is sized by
its bind pose, not by the pose it is animated into). A mesh past either is culled;
with both set, whichever culls first.

- **Per camera, per frame.** Each camera measures from itself: the main view, a
  stacked camera, a render-texture camera. Whether a mesh was culled is the
  renderer's and is not readable from scripts: the simulation never depends on it.
  A culled mesh still collides, animates and runs its scripts.
- **Shadows go with the prop, as the main camera sees it.** Shadow maps are drawn
  once per frame, measured from the **main (base) camera**, never the light: a prop
  that camera culls casts nothing, and one it draws keeps its shadow, so the shadow
  never pops on its own. Another camera (a stacked one, a render-texture CCTV
  screen) has its own verdict for the prop but shares those shadows, so it can
  show a prop the main camera culled, with no shadow under it. A mesh with either
  setting is never baked into the cached static shadows (the sun's static
  cascades, a light's static atlas tiles): it casts from the per-frame shadow pass
  even when Static, so its shadow can follow the camera. Turning a setting on or
  off re-bakes them. A `ShadowsOnly` mesh (no camera draws it,
  [`Light`](Light.md#cast-shadows)) is measured the same way for its shadow.
- **LODGroup.** A level its group hides is skipped before this test; a level the
  group shows is still culled by its own renderer's settings, whichever culls
  first. When any renderer of a group sets either, **every** renderer of that
  group leaves the static shadow bake, Static or not: the bake draws a group at
  LOD0 only, so a group split between the bake and the per-frame pass would leave
  a level with no shadow, or with two.
- **Bakes see everything.** Light-probe, reflection-probe and lightmap captures
  cull nothing by these settings: a probe on the far side of a prop still sees it.
- **The cost.** The test runs only on meshes the frustum kept: a branch with both
  settings off, a distance and a division with one on, nothing for what the
  frustum culled. A culled mesh is never drawn, in a camera view or a shadow map,
  but the shadow pass still gathers it each frame to test it (its material runs
  and one entry per run; a skinned one uploads no bone palette). A prop pops in
  and out at the threshold; there is no fade.

`Debug.Stats()` counts it per frame ([`Debug`](Debug.md)): `cull_tested`,
`culled_entities` and `size_culled_entities` over the camera views, and
`shadow_cull_tested`, `shadow_frustum_culled` and `shadow_size_culled` over the
shadow views.

```lua
-- A can on a shelf: gone past 30 m, or under 2% of the screen.
local can = Scene.CreateEntity("Can", "Cylinder")
Mesh.SetMaxDrawDistance(can, 30)
Mesh.SetMinScreenSize(can, 0.02)
print(Mesh.GetMaxDrawDistance(can)) --> 30.0
```

The values are stored as 32-bit floats, so `Mesh.GetMinScreenSize(can)` reads back
`0.019999999552965`, not `0.02`: compare with a tolerance, never with `==`.
