## `Mesh`

Per-renderer settings of an entity's `MeshComponent` (#1714, `src/api/mesh.rs`):
Unity's *Mesh Renderer*. Today they are the **small-object cull**: a prop that is
too far away, or too small on screen, is not drawn. A 20 cm can on a shelf across
a 50 m hall covers a few pixels: drawing it costs a draw, its shadow and its
lighting, and shows nothing. Both settings are off by default, so a scene that sets
neither draws exactly what it drew before.

| Function | Arguments | Returns |
|---|---|---|
| `Mesh.GetMaxDrawDistance` | `(id)` | the distance in metres past which a camera stops drawing it; `0` = unlimited |
| `Mesh.SetMaxDrawDistance` | `(id, metres)` | — negative, NaN or infinite is `0` |
| `Mesh.GetMinScreenSize` | `(id)` | the screen-relative height (`0..1`) under which a camera stops drawing it; `0` = off |
| `Mesh.SetMinScreenSize` | `(id, height)` | — held to `[0, 1]`; NaN or infinite is `0` |

Without a Mesh, getters return `0` and setters do nothing. The same fields are
`max_draw_distance` and `min_screen_size` to `Component.Get` / `Component.Set(id,
"Mesh", { … })` ([`Component`](Component.md)), the Inspector's Mesh card shows them,
and the scene and its prefabs save them when set.

**Max Draw Distance** is Unreal's: measured from the camera to the centre of the
mesh's world bounds. **Min Screen Size** is the measure Unity's `LODGroup` picks
levels by ([`LODGroup`](LODGroup.md)), and so the same value as its *Culled*
share: the largest extent of the mesh's world bounds over the height the view
spans at that distance, `size / (2 · distance · tan(fov / 2))`; an orthographic
camera measures `size / (2 · half-height)` at any distance. A mesh past either is
culled; with both set, whichever culls first.

- **Per camera, per frame.** Each camera measures from itself: the main view, a
  stacked camera, a render-texture camera. Whether a mesh was culled is the
  renderer's and is not readable from scripts: the simulation never depends on it.
  A culled mesh still collides, animates and runs its scripts.
- **Shadows go with the prop.** The shadow views measure from the **main camera**,
  never the light, so a culled prop casts nothing and a drawn one keeps its shadow:
  the shadow never pops on its own. A mesh with either setting is never baked
  into the cached static shadows (the sun's static cascades, a light's static
  atlas tiles): it casts from the per-frame shadow pass even when Static, so its
  shadow can follow the camera. Turning a setting on or off re-bakes them. A
  `ShadowsOnly` mesh (no camera draws it, [`Light`](Light.md#cast-shadows)) is
  measured the same way for its shadow.
- **LODGroup.** A level its group hides is skipped before this test; a level the
  group shows is still culled by its own renderer's settings. The two combine as
  whichever culls first.
- **Bakes see everything.** Light-probe, reflection-probe and lightmap captures
  cull nothing by these settings: a probe on the far side of a prop still sees it.
- **The cost.** The test runs only on meshes the frustum kept: a branch with both
  settings off, a distance and a division with one on, nothing for what the
  frustum culled. A prop pops in and out at the threshold; there is no fade.

`Debug.Stats()` counts it per frame ([`Debug`](Debug.md)): `cull_tested`,
`culled_entities` and `size_culled_entities` over the camera views, and
`shadow_cull_tested`, `shadow_frustum_culled` and `shadow_size_culled` over the
shadow views.

```lua
-- A can on a shelf: gone past 30 m, or under 2% of the screen.
local can = Scene.CreateEntity("Can", "Cylinder")
Mesh.SetMaxDrawDistance(can, 30)
Mesh.SetMinScreenSize(can, 0.02)
print(Mesh.GetMaxDrawDistance(can), Mesh.GetMinScreenSize(can)) --> 30  0.02
```
